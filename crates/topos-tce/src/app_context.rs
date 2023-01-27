#![allow(unused_variables)]
//!
//! Application logic glue
//!
use futures::FutureExt;
use futures::{future::join_all, Stream, StreamExt};
use opentelemetry::trace::FutureExt as TraceFutureExt;
use serde::{Deserialize, Serialize};
use tce_transport::{TceCommands, TceEvents};
use tokio::spawn;
use tokio::sync::oneshot;
use topos_core::uci::CertificateId;
use topos_p2p::{Client as NetworkClient, Event as NetEvent, RetryPolicy};
use topos_tce_api::RuntimeEvent as ApiEvent;
use topos_tce_api::{RuntimeClient as ApiClient, RuntimeError};
use topos_tce_broadcast::sampler::SampleType;
use topos_tce_broadcast::DoubleEchoCommand;
use topos_tce_broadcast::{ReliableBroadcastClient, SamplerCommand};
use topos_tce_gatekeeper::{GatekeeperClient, GatekeeperError};
use topos_tce_storage::errors::{InternalStorageError, StorageError};
use topos_tce_storage::events::StorageEvent;
use topos_tce_storage::StorageClient;
use topos_tce_synchronizer::{SynchronizerClient, SynchronizerEvent};
use topos_telemetry::PropagationContext;
use tracing::{debug, error, info, info_span, trace, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Top-level transducer main app context & driver (alike)
///
/// Implements <...Host> traits for network and Api, listens for protocol events in events
/// (store is not active component).
///
/// In the end we shall come to design where this struct receives
/// config+data as input and runs app returning data as output
///
pub struct AppContext {
    pub tce_cli: ReliableBroadcastClient,
    pub network_client: NetworkClient,
    pub api_client: ApiClient,
    pub pending_storage: StorageClient,
    pub gatekeeper: GatekeeperClient,
    pub synchronizer: SynchronizerClient,
}

impl AppContext {
    // Default previous certificate id for first certificate in the subnet
    //TODO Remove, it will be genesis certificate id retrieved from Topos Subnet
    const DUMMY_INITIAL_CERTIFICATE_ID: CertificateId = CertificateId::from_array([0u8; 32]);

    /// Factory
    pub fn new(
        pending_storage: StorageClient,
        tce_cli: ReliableBroadcastClient,
        network_client: NetworkClient,
        api_client: ApiClient,
        gatekeeper: GatekeeperClient,
        synchronizer: SynchronizerClient,
    ) -> Self {
        Self {
            tce_cli,
            network_client,
            api_client,
            pending_storage,
            gatekeeper,
            synchronizer,
        }
    }

    /// Main processing loop
    pub async fn run(
        mut self,
        mut network_stream: impl Stream<Item = NetEvent> + Unpin,
        mut tce_stream: impl Stream<Item = Result<TceEvents, ()>> + Unpin,
        mut api_stream: impl Stream<Item = ApiEvent> + Unpin,
        mut storage_stream: impl Stream<Item = StorageEvent> + Unpin,
        mut synchronizer_stream: impl Stream<Item = SynchronizerEvent> + Unpin,
    ) {
        loop {
            tokio::select! {

                // protocol
                Some(Ok(evt)) = tce_stream.next() => {
                    self.on_protocol_event(evt).await;
                },

                // network
                Some(net_evt) = network_stream.next() => {
                    self.on_net_event(net_evt).await;
                }

                // api events
                Some(event) = api_stream.next() => {
                    self.on_api_event(event).await;
                }

                // Storage events
                Some(_event) = storage_stream.next() => {
                }

                // Synchronizer events
                Some(_event) = synchronizer_stream.next() => {
                }
            }
        }
    }

    async fn on_api_event(&mut self, event: ApiEvent) {
        match event {
            ApiEvent::CertificateSubmitted {
                certificate,
                sender,
                ctx,
            } => {
                let span = info_span!(parent: &ctx, "TCE Runtime");

                async {
                    _ = self
                        .pending_storage
                        .add_pending_certificate(certificate.clone())
                        .instrument(Span::current())
                        .await;
                    info!("Certificate added to pending storage");
                    spawn(
                        self.tce_cli
                            .broadcast_new_certificate(certificate, Span::current())
                            .instrument(Span::current()),
                    );
                    _ = sender.send(Ok(()));
                }
                .instrument(span)
                // .instrument(span)
                .await;
            }

            ApiEvent::PeerListPushed { peers, sender } => {
                let sampler = self.tce_cli.clone();

                match self.gatekeeper.push_peer_list(peers).await {
                    Ok(peers) => {
                        if sampler.peer_changed(peers).await.is_err() {
                            _ = sender.send(Err(RuntimeError::UnableToPushPeerList));
                        } else {
                            _ = sender.send(Ok(()));
                        }
                    }
                    Err(GatekeeperError::NoUpdate) => {
                        _ = sender.send(Ok(()));
                    }
                    Err(_) => {
                        _ = sender.send(Err(RuntimeError::UnableToPushPeerList));
                    }
                }
            }

            ApiEvent::GetSourceHead { subnet_id, sender } => {
                // Get source head certificate
                let mut result = self
                    .pending_storage
                    .get_source_head(subnet_id)
                    .await
                    .map_err(|e| match e {
                        StorageError::InternalStorage(internal) => {
                            if let InternalStorageError::MissingHeadForSubnet(subnet_id) = internal
                            {
                                RuntimeError::UnknownSubnet(subnet_id)
                            } else {
                                RuntimeError::UnableToGetSourceHead(subnet_id, internal.to_string())
                            }
                        }
                        e => RuntimeError::UnableToGetSourceHead(subnet_id, e.to_string()),
                    });

                //TODO Initial genesis certificate eventually will be fetched from the topos subnet
                //Currently, for subnet starting from scratch there are no certificates in the database
                // So for MissingHeadForSubnet error we will return some default dummy certificate
                if let Err(RuntimeError::UnknownSubnet(subnet_id)) = result {
                    result = Ok((
                        0,
                        topos_core::uci::Certificate {
                            id: AppContext::DUMMY_INITIAL_CERTIFICATE_ID,
                            prev_id: AppContext::DUMMY_INITIAL_CERTIFICATE_ID,
                            source_subnet_id: subnet_id,
                            target_subnets: vec![],
                        },
                    ));
                };

                _ = sender.send(result);
            }
        }
    }

    async fn on_protocol_event(&mut self, evt: TceEvents) {
        match evt {
            TceEvents::StableSample => {
                info!("Stable Sample detected");
                self.api_client.set_active_sample(true).await;
            }

            TceEvents::CertificateDelivered { certificate } => {
                _ = self
                    .pending_storage
                    .certificate_delivered(certificate.id)
                    .await;
                spawn(self.api_client.dispatch_certificate(certificate));
            }

            TceEvents::EchoSubscribeReq { peers } => {
                // Preparing echo subscribe message
                let my_peer_id = self.network_client.local_peer_id;
                let data: Vec<u8> = NetworkMessage::from(TceCommands::OnEchoSubscribeReq {
                    from_peer: self.network_client.local_peer_id,
                })
                .into();
                let command_sender = self.tce_cli.get_sampler_channel();
                // Sending echo subscribe message to send to a number of remote peers
                let future_pool = peers
                    .iter()
                    .map(|peer_id| {
                        debug!(
                            "peer_id: {} sending echo subscribe to {}",
                            &my_peer_id, &peer_id
                        );
                        let peer_id = *peer_id;
                        self.network_client
                            .send_request::<_, NetworkMessage>(
                                peer_id,
                                data.clone(),
                                RetryPolicy::N(3),
                            )
                            .map(move |result| (peer_id, result))
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    // Waiting for all responses from remote peers on our echo subscription request
                    let results = join_all(future_pool).await;

                    // Process responses
                    for (peer_id, result) in results {
                        match result {
                            Ok(message) => match message {
                                // Remote peer has replied us that he is accepting us as echo subscriber
                                NetworkMessage::Cmd(TceCommands::OnEchoSubscribeOk {
                                    from_peer,
                                }) => {
                                    debug!("Receive response to EchoSubscribe",);
                                    let (sender, receiver) = oneshot::channel();
                                    let _ = command_sender
                                        .send(SamplerCommand::ConfirmPeer {
                                            peer: from_peer,
                                            sample_type: SampleType::EchoSubscription,
                                            sender,
                                        })
                                        .await;

                                    let _ = receiver.await.expect("Sender was dropped");
                                }
                                msg => {
                                    error!("Receive an unexpected message as a response {msg:?}");
                                }
                            },
                            Err(error) => {
                                error!("An error occurred when sending EchoSubscribe {error:?} for peer {peer_id}");
                            }
                        }
                    }
                });
            }
            TceEvents::ReadySubscribeReq { peers } => {
                // Preparing ready subscribe message
                let my_peer_id = self.network_client.local_peer_id;
                let data: Vec<u8> = NetworkMessage::from(TceCommands::OnReadySubscribeReq {
                    from_peer: self.network_client.local_peer_id,
                })
                .into();
                let command_sender = self.tce_cli.get_sampler_channel();
                // Sending ready subscribe message to send to a number of remote peers
                let future_pool = peers
                    .into_iter()
                    .map(|peer_id| {
                        debug!(
                            "peer_id: {} sending ready subscribe to {}",
                            &my_peer_id, &peer_id
                        );
                        self.network_client
                            .send_request::<_, NetworkMessage>(
                                peer_id,
                                data.clone(),
                                RetryPolicy::N(3),
                            )
                            .map(move |result| (peer_id, result))
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    // Waiting for all responses from remote peers on our ready subscription request
                    let results = join_all(future_pool).await;

                    // Process responses from remote peers
                    for (peer_id, result) in results {
                        match result {
                            Ok(message) => match message {
                                // Remote peer has replied us that he is accepting us as ready subscriber
                                NetworkMessage::Cmd(TceCommands::OnReadySubscribeOk {
                                    from_peer,
                                }) => {
                                    debug!("Receive response to ReadySubscribe");
                                    let (sender_ready, receiver_ready) = oneshot::channel();
                                    let _ = command_sender
                                        .send(SamplerCommand::ConfirmPeer {
                                            peer: from_peer,
                                            sample_type: SampleType::ReadySubscription,
                                            sender: sender_ready,
                                        })
                                        .await;
                                    let (sender_delivery, receiver_delivery) = oneshot::channel();
                                    let _ = command_sender
                                        .send(SamplerCommand::ConfirmPeer {
                                            peer: from_peer,
                                            sample_type: SampleType::DeliverySubscription,
                                            sender: sender_delivery,
                                        })
                                        .await;

                                    join_all(vec![receiver_ready, receiver_delivery]).await;
                                }
                                msg => {
                                    error!("Receive an unexpected message as a response {msg:?}");
                                }
                            },
                            Err(error) => {
                                error!("An error occurred when sending ReadySubscribe {error:?} for peer {peer_id}");
                            }
                        }
                    }
                });
            }

            TceEvents::Gossip {
                peers, cert, ctx, ..
            } => {
                let cert_id = cert.id;

                let data: Vec<u8> = NetworkMessage::from(TceCommands::OnGossip {
                    cert,
                    digest: vec![],
                    ctx: PropagationContext::inject(&ctx),
                })
                .into();

                let future_pool = peers
                    .iter()
                    .map(|peer_id| {
                        debug!(
                            "peer_id: {} sending gossip cert id: {} to peer {}",
                            &self.network_client.local_peer_id, &cert_id, &peer_id
                        );
                        self.network_client.send_request::<_, NetworkMessage>(
                            *peer_id,
                            data.clone(),
                            RetryPolicy::N(3),
                        )
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    let _results = join_all(future_pool).await;
                });
            }

            TceEvents::Echo { peers, cert, ctx } => {
                let my_peer_id = self.network_client.local_peer_id;
                debug!(
                    "peer_id: {} processing on_protocol_event TceEvents::Echo peers {:?} cert id: {}",
                    &my_peer_id, &peers, &cert.id
                );
                // Send echo message
                let data: Vec<u8> = NetworkMessage::from(TceCommands::OnEcho {
                    from_peer: self.network_client.local_peer_id,
                    cert,
                    ctx: PropagationContext::inject(&ctx),
                })
                .into();

                let future_pool = peers
                    .iter()
                    .map(|peer_id| {
                        debug!("peer_id: {} sending Echo to {}", &my_peer_id, &peer_id);
                        self.network_client.send_request::<_, NetworkMessage>(
                            *peer_id,
                            data.clone(),
                            RetryPolicy::N(3),
                        )
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    let _results = join_all(future_pool).await;
                });
            }

            TceEvents::Ready { peers, cert, ctx } => {
                let my_peer_id = self.network_client.local_peer_id;
                debug!(
                    "peer_id: {} processing TceEvents::Ready peers {:?} cert id: {}",
                    &my_peer_id, &peers, &cert.id
                );
                let data: Vec<u8> = NetworkMessage::from(TceCommands::OnReady {
                    from_peer: self.network_client.local_peer_id,
                    cert,
                    ctx: PropagationContext::inject(&ctx),
                })
                .into();

                let future_pool = peers
                    .iter()
                    .map(|peer_id| {
                        debug!("peer_id: {} sending Ready to {}", &my_peer_id, &peer_id);
                        self.network_client.send_request::<_, NetworkMessage>(
                            *peer_id,
                            data.clone(),
                            RetryPolicy::N(3),
                        )
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    let _results = join_all(future_pool).await;
                });
            }
            evt => {
                debug!("Unhandled event: {:?}", evt);
            }
        }
    }

    async fn on_net_event(&mut self, evt: NetEvent) {
        trace!(
            "on_net_event: peer: {} event {:?}",
            &self.network_client.local_peer_id,
            &evt
        );
        match evt {
            NetEvent::PeersChanged { .. } => {}

            NetEvent::TransmissionOnReq {
                from,
                data,
                channel,
                ..
            } => {
                let my_peer = self.network_client.local_peer_id;
                let msg: NetworkMessage = data.into();
                match msg {
                    NetworkMessage::NotReady(_) => {}
                    NetworkMessage::Cmd(cmd) => {
                        match cmd {
                            // We received echo subscription request from external peer
                            TceCommands::OnEchoSubscribeReq { from_peer } => {
                                debug!(
                                    sender = from.to_string(),
                                    "on_net_event peer {} TceCommands::OnEchoSubscribeReq from_peer: {}",
                                    &self.network_client.local_peer_id, &from_peer
                                );
                                self.tce_cli
                                    .add_confirmed_peer_to_sample(
                                        SampleType::EchoSubscriber,
                                        from_peer,
                                    )
                                    .await;

                                // We are responding that we are accepting echo subscriber
                                spawn(self.network_client.respond_to_request(
                                    NetworkMessage::from(TceCommands::OnEchoSubscribeOk {
                                        from_peer: my_peer,
                                    }),
                                    channel,
                                ));
                            }

                            // We received ready subscription request from external peer
                            TceCommands::OnReadySubscribeReq { from_peer } => {
                                debug!(
                                    sender = from.to_string(),
                                    "peer_id {} on_net_event TceCommands::OnReadySubscribeReq from_peer: {}",
                                    &self.network_client.local_peer_id, &from_peer,
                                );
                                self.tce_cli
                                    .add_confirmed_peer_to_sample(
                                        SampleType::ReadySubscriber,
                                        from_peer,
                                    )
                                    .await;
                                // We are responding that we are accepting ready subscriber
                                spawn(self.network_client.respond_to_request(
                                    NetworkMessage::from(TceCommands::OnReadySubscribeOk {
                                        from_peer: my_peer,
                                    }),
                                    channel,
                                ));
                            }

                            TceCommands::OnGossip {
                                cert,
                                digest: _,
                                ctx,
                            } => {
                                let span = info_span!(
                                    "Gossip",
                                    peer_id = self.network_client.local_peer_id.to_string(),
                                    "otel.kind" = "consumer",
                                    sender = from.to_string()
                                );
                                // warn!("RECV GOSSIP context: {:?}", ctx);
                                let parent = ctx.extract();
                                span.set_parent(parent);
                                span.in_scope(|| {
                                    debug!(
                                        sender = from.to_string(),
                                        "peer_id {} on_net_event TceCommands::OnGossip cert id: {}",
                                        &self.network_client.local_peer_id,
                                        &cert.id,
                                    );
                                });

                                _ = self
                                    .pending_storage
                                    .add_pending_certificate(cert.clone())
                                    .await;

                                let command_sender = self.tce_cli.get_double_echo_channel();
                                command_sender
                                    .send(DoubleEchoCommand::Broadcast {
                                        cert,
                                        ctx: span.clone(),
                                    })
                                    .await
                                    .expect("Gossip the certificate");

                                spawn(self.network_client.respond_to_request(
                                    NetworkMessage::from(TceCommands::OnDoubleEchoOk {
                                        from_peer: my_peer,
                                    }),
                                    channel,
                                ));
                            }
                            TceCommands::OnEcho {
                                from_peer,
                                cert,
                                ctx,
                                ..
                            } => {
                                let span = info_span!(
                                    "Echo",
                                    peer_id = self.network_client.local_peer_id.to_string(),
                                    "otel.kind" = "consumer",
                                    sender = from.to_string()
                                );
                                // warn!("RECV Echo context: {:?}", ctx);
                                let context = ctx.extract();
                                span.set_parent(context.clone());
                                // We have received Echo echo message, we are responding with OnDoubleEchoOk
                                let command_sender = span.in_scope(||{
                                    info!(
                                    sender = from.to_string(),
                                        "peer_id: {} on_net_event TceCommands::OnEcho from peer {} cert id: {}",
                                        &self.network_client.local_peer_id, &from_peer, &cert.id
                                    );
                                    // We have received echo message from external peer
                                    self.tce_cli.get_double_echo_channel()
                                });
                                command_sender
                                    .send(DoubleEchoCommand::Echo {
                                        from_peer,
                                        cert,
                                        ctx: span.clone(),
                                    })
                                    .with_context(context)
                                    .instrument(span)
                                    .await
                                    .expect("Receive the Echo");
                                //We are responding with OnDoubleEchoOk to remote peer
                                spawn(self.network_client.respond_to_request(
                                    NetworkMessage::from(TceCommands::OnDoubleEchoOk {
                                        from_peer: my_peer,
                                    }),
                                    channel,
                                ));
                            }
                            TceCommands::OnReady {
                                from_peer,
                                cert,
                                ctx,
                            } => {
                                let span = info_span!(
                                    "Ready",
                                    peer_id = self.network_client.local_peer_id.to_string(),
                                    sender = from.to_string()
                                );
                                // warn!("RECV Ready context: {:?}", ctx);
                                let context = ctx.extract();
                                span.set_parent(context.clone());
                                let command_sender = span.in_scope(||{

                                    // We have received Ready echo message, we are responding with OnDoubleEchoOk
                                    info!(
                                    sender = from.to_string(),
                                        "peer_id {} on_net_event TceCommands::OnReady from peer {} cert id: {}",
                                        &self.network_client.local_peer_id, &from_peer, &cert.id
                                    );
                                    self.tce_cli.get_double_echo_channel()
                                });
                                command_sender
                                    .send(DoubleEchoCommand::Ready {
                                        from_peer,
                                        cert,
                                        ctx: span.clone(),
                                    })
                                    .with_context(context)
                                    .instrument(span)
                                    .await
                                    .expect("Receive the Ready");

                                spawn(self.network_client.respond_to_request(
                                    NetworkMessage::from(TceCommands::OnDoubleEchoOk {
                                        from_peer: my_peer,
                                    }),
                                    channel,
                                ));
                            }
                            _ => todo!(),
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Definition of networking payload.
///
/// We assume that only Commands will go through the network,
/// [Response] is used to allow reporting of logic errors to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
enum NetworkMessage {
    Cmd(TceCommands),

    NotReady(topos_p2p::NotReadyMessage),
}

// deserializer
impl From<Vec<u8>> for NetworkMessage {
    fn from(data: Vec<u8>) -> Self {
        bincode::deserialize::<NetworkMessage>(data.as_ref()).expect("msg deser")
    }
}

// serializer
impl From<NetworkMessage> for Vec<u8> {
    fn from(msg: NetworkMessage) -> Self {
        bincode::serialize::<NetworkMessage>(&msg).expect("msg ser")
    }
}

// transformer of protocol commands into network commands
impl From<TceCommands> for NetworkMessage {
    fn from(cmd: TceCommands) -> Self {
        Self::Cmd(cmd)
    }
}
