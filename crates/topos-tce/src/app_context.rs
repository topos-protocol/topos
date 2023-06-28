#![allow(unused_variables)]
//!
//! Application logic glue
//!
use futures::{Stream, StreamExt};
use opentelemetry::trace::{FutureExt as TraceFutureExt, TraceContextExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tce_transport::{ProtocolEvents, TceCommands};
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};
use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_metrics::CERTIFICATE_DELIVERED;
use topos_p2p::{Client as NetworkClient, Event as NetEvent};
use topos_tce_api::RuntimeEvent as ApiEvent;
use topos_tce_api::{RuntimeClient as ApiClient, RuntimeError};
use topos_tce_broadcast::DoubleEchoCommand;
use topos_tce_broadcast::ReliableBroadcastClient;
use topos_tce_gatekeeper::{GatekeeperClient, GatekeeperError};
use topos_tce_storage::errors::{InternalStorageError, StorageError};
use topos_tce_storage::events::StorageEvent;
use topos_tce_storage::StorageClient;
use topos_tce_synchronizer::{SynchronizerClient, SynchronizerEvent};
use topos_telemetry::PropagationContext;
use tracing::{debug, error, info, info_span, trace, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::events::Events;

/// Top-level transducer main app context & driver (alike)
///
/// Implements <...Host> traits for network and Api, listens for protocol events in events
/// (store is not active component).
///
/// In the end we shall come to design where this struct receives
/// config+data as input and runs app returning data as output
///
pub struct AppContext {
    pub events: mpsc::Sender<Events>,
    pub tce_cli: ReliableBroadcastClient,
    pub network_client: NetworkClient,
    pub api_client: ApiClient,
    pub pending_storage: StorageClient,
    pub gatekeeper: GatekeeperClient,
    pub synchronizer: SynchronizerClient,
}

impl AppContext {
    // Default previous certificate id for first certificate in the subnet
    // TODO: Remove, it will be genesis certificate id retrieved from Topos Subnet
    const DUMMY_INITIAL_CERTIFICATE_ID: CertificateId =
        CertificateId::from_array([0u8; topos_core::uci::CERTIFICATE_ID_LENGTH]);

    /// Factory
    pub fn new(
        pending_storage: StorageClient,
        tce_cli: ReliableBroadcastClient,
        network_client: NetworkClient,
        api_client: ApiClient,
        gatekeeper: GatekeeperClient,
        synchronizer: SynchronizerClient,
    ) -> (Self, mpsc::Receiver<Events>) {
        let (events, receiver) = mpsc::channel(100);
        (
            Self {
                events,
                tce_cli,
                network_client,
                api_client,
                pending_storage,
                gatekeeper,
                synchronizer,
            },
            receiver,
        )
    }

    /// Main processing loop
    pub async fn run(
        mut self,
        mut network_stream: impl Stream<Item = NetEvent> + Unpin,
        mut tce_stream: impl Stream<Item = Result<ProtocolEvents, ()>> + Unpin,
        mut api_stream: impl Stream<Item = ApiEvent> + Unpin,
        mut storage_stream: impl Stream<Item = StorageEvent> + Unpin,
        mut synchronizer_stream: impl Stream<Item = SynchronizerEvent> + Unpin,
        mut shutdown: mpsc::Receiver<oneshot::Sender<()>>,
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

                // Shutdown signal
                Some(sender) = shutdown.recv() => {
                    info!("Shutting down TCE app context...");
                    if let Err(e) = self.shutdown().await {
                        error!("Error shutting down TCE app context: {e}");
                    }
                    // Send feedback that shutdown has been finished
                    _ = sender.send(());
                    break;
                }
            }
        }
        warn!("Exiting main TCE app processing loop")
    }

    async fn on_api_event(&mut self, event: ApiEvent) {
        match event {
            ApiEvent::CertificateSubmitted {
                certificate,
                sender,
                ctx,
            } => {
                let span = info_span!(parent: &ctx, "TCE Runtime");

                _ = self
                    .tce_cli
                    .broadcast_new_certificate(*certificate, true)
                    .with_context(span.context())
                    .instrument(span)
                    .await;

                _ = sender.send(Ok(()));
            }

            ApiEvent::PeerListPushed { peers, sender } => {
                let sampler = self.tce_cli.clone();
                let gatekeeper = self.gatekeeper.clone();
                let events = self.events.clone();
                let api = self.api_client.clone();

                spawn(async move {
                    match gatekeeper.push_peer_list(peers).await {
                        Ok(peers) => {
                            info!("Gatekeeper has detected changes on the peer list, new sample in creation");
                            if sampler.peer_changed(peers).await.is_err() {
                                _ = sender.send(Err(RuntimeError::UnableToPushPeerList));
                            } else {
                                api.set_active_sample(true).await;
                                if events.send(Events::StableSample).await.is_err() {
                                    error!("Unable to send StableSample event");
                                }
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
                });
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

                // TODO: Initial genesis certificate eventually will be fetched from the topos subnet
                // Currently, for subnet starting from scratch there are no certificates in the database
                // So for MissingHeadForSubnet error we will return some default dummy certificate
                if let Err(RuntimeError::UnknownSubnet(subnet_id)) = result {
                    warn!("Returning dummy certificate as head certificate, to be fixed...");
                    result = Ok((
                        0,
                        topos_core::uci::Certificate {
                            prev_id: AppContext::DUMMY_INITIAL_CERTIFICATE_ID,
                            source_subnet_id: subnet_id,
                            state_root: Default::default(),
                            tx_root_hash: Default::default(),
                            target_subnets: vec![],
                            verifier: 0,
                            id: AppContext::DUMMY_INITIAL_CERTIFICATE_ID,
                            proof: Default::default(),
                            signature: Default::default(),
                        },
                    ));
                };

                _ = sender.send(result);
            }

            ApiEvent::GetLastPendingCertificates {
                mut subnet_ids,
                sender,
            } => {
                let mut last_pending_certificates: HashMap<SubnetId, Option<Certificate>> =
                    subnet_ids
                        .iter()
                        .map(|subnet_id| (*subnet_id, None))
                        .collect();

                if let Ok(pending_certificates) =
                    self.pending_storage.get_pending_certificates().await
                {
                    // Iterate through pending certificates and determine last one for every subnet
                    // Last certificate in the subnet should be one with the highest index
                    for (pending_certificate_id, cert) in pending_certificates.into_iter().rev() {
                        if let Some(subnet_id) = subnet_ids.take(&cert.source_subnet_id) {
                            *last_pending_certificates.entry(subnet_id).or_insert(None) =
                                Some(cert);
                        }
                        if subnet_ids.is_empty() {
                            break;
                        }
                    }
                }

                // Add None pending certificate for any other requested subnet_id
                subnet_ids.iter().for_each(|subnet_id| {
                    last_pending_certificates.insert(*subnet_id, None);
                });

                _ = sender.send(Ok(last_pending_certificates));
            }
        }
    }

    async fn on_protocol_event(&mut self, evt: ProtocolEvents) {
        match evt {
            ProtocolEvents::StableSample => {
                info!("Stable Sample detected");
                self.api_client.set_active_sample(true).await;
                if self.events.send(Events::StableSample).await.is_err() {
                    error!("Unable to send StableSample event");
                }
            }

            ProtocolEvents::Broadcast { certificate_id } => {
                info!("Broadcasting certificate {}", certificate_id);
            }

            ProtocolEvents::CertificateDelivered { certificate } => {
                warn!("Certificate delivered {}", certificate.id);
                CERTIFICATE_DELIVERED.inc();
                let storage = self.pending_storage.clone();
                let api_client = self.api_client.clone();

                spawn(async move {
                    match storage.certificate_delivered(certificate.id).await {
                        Ok(positions) => {
                            let certificate_id = certificate.id;
                            api_client
                                .dispatch_certificate(
                                    certificate,
                                    positions
                                        .targets
                                        .into_iter()
                                        .map(|(subnet_id, certificate_target_stream_position)| {
                                            (
                                                subnet_id,
                                                TargetStreamPosition {
                                                    target_subnet_id:
                                                        certificate_target_stream_position
                                                            .target_subnet_id,
                                                    source_subnet_id:
                                                        certificate_target_stream_position
                                                            .source_subnet_id,
                                                    position: certificate_target_stream_position
                                                        .position
                                                        .0,
                                                    certificate_id: Some(certificate_id),
                                                },
                                            )
                                        })
                                        .collect::<HashMap<SubnetId, TargetStreamPosition>>(),
                                )
                                .await;
                        }
                        Err(StorageError::InternalStorage(
                            InternalStorageError::CertificateNotFound(_),
                        )) => {}
                        Err(e) => {
                            error!("Pending storage error while delivering certificate: {e}");
                        }
                    };
                });
            }

            ProtocolEvents::Gossip { cert, ctx } => {
                let span = info_span!(
                    parent: &ctx,
                    "SEND Outbound Gossip",
                    peer_id = self.network_client.local_peer_id.to_string(),
                    "otel.kind" = "producer",
                );
                let cert_id = cert.id;

                let data = NetworkMessage::from(TceCommands::OnGossip {
                    cert,
                    ctx: PropagationContext::inject(&span.context()),
                });

                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_GOSSIP, data)
                    .await
                {
                    error!("Unable to send Gossip due to error: {e}");
                }
            }

            ProtocolEvents::Echo {
                certificate_id,
                ctx,
            } => {
                let span = info_span!(
                    parent: &ctx,
                    "SEND Outbound Echo",
                    peer_id = self.network_client.local_peer_id.to_string(),
                    "otel.kind" = "producer",
                );
                let my_peer_id = self.network_client.local_peer_id;
                // Send echo message
                let data = NetworkMessage::from(TceCommands::OnEcho {
                    certificate_id,
                    ctx: PropagationContext::inject(&span.context()),
                });

                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_ECHO, data)
                    .await
                {
                    error!("Unable to send Echo due to error: {e}");
                }
            }

            ProtocolEvents::Ready {
                certificate_id,
                ctx,
            } => {
                let span = info_span!(
                    parent: &ctx,
                    "SEND Outbound Ready",
                    peer_id = self.network_client.local_peer_id.to_string(),
                    "otel.kind" = "producer",
                );
                let my_peer_id = self.network_client.local_peer_id;
                let data = NetworkMessage::from(TceCommands::OnReady {
                    certificate_id,
                    ctx: PropagationContext::inject(&span.context()),
                });

                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_READY, data)
                    .await
                {
                    error!("Unable to send Ready due to error: {e}");
                }
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

        if let NetEvent::Gossip { from, data } = evt {
            let msg: NetworkMessage = data.into();

            if let NetworkMessage::Cmd(cmd) = msg {
                match cmd {
                    TceCommands::OnGossip { cert, ctx } => {
                        let span = info_span!(
                            "RECV Outbound Gossip",
                            peer_id = self.network_client.local_peer_id.to_string(),
                            "otel.kind" = "consumer",
                            sender = from.to_string()
                        );
                        let parent = ctx.extract();
                        span.add_link(parent.span().span_context().clone());

                        let channel = self.tce_cli.get_double_echo_channel();

                        spawn(async move {
                            info!("Send certificate to be broadcast");
                            if channel
                                .send(DoubleEchoCommand::Broadcast {
                                    cert,
                                    need_gossip: false,
                                    ctx: span,
                                })
                                .await
                                .is_err()
                            {
                                error!("Unable to send broadcast_new_certificate command, Receiver was dropped");
                            }
                        });
                    }

                    TceCommands::OnEcho {
                        certificate_id,
                        ctx,
                    } => {
                        let span = info_span!(
                            "RECV Outbound Echo",
                            peer_id = self.network_client.local_peer_id.to_string(),
                            "otel.kind" = "consumer",
                            sender = from.to_string()
                        );
                        let context = ctx.extract();
                        span.add_link(context.span().span_context().clone());

                        let channel = self.tce_cli.get_double_echo_channel();
                        spawn(async move {
                            if let Err(e) = channel
                                .send(DoubleEchoCommand::Echo {
                                    from_peer: from,
                                    certificate_id,
                                    ctx: span.clone(),
                                })
                                .with_context(span.context().clone())
                                .instrument(span)
                                .await
                            {
                                error!("Unable to send Echo, {:?}", e);
                            }
                        });
                    }
                    TceCommands::OnReady {
                        certificate_id,
                        ctx,
                    } => {
                        let span = info_span!(
                            "RECV Outbound Ready",
                            peer_id = self.network_client.local_peer_id.to_string(),
                            "otel.kind" = "consumer",
                            sender = from.to_string()
                        );
                        let context = ctx.extract();
                        span.add_link(context.span().span_context().clone());

                        let channel = self.tce_cli.get_double_echo_channel();
                        spawn(async move {
                            if let Err(e) = channel
                                .send(DoubleEchoCommand::Ready {
                                    from_peer: from,
                                    certificate_id,
                                    ctx: span.clone(),
                                })
                                .with_context(context)
                                .instrument(span)
                                .await
                            {
                                error!("Unable to send Ready {:?}", e);
                            }
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Shutting down the TCE client...");
        self.api_client.shutdown().await?;
        self.synchronizer.shutdown().await?;
        self.pending_storage.shutdown().await?;
        self.tce_cli.shutdown().await?;
        self.gatekeeper.shutdown().await?;
        self.network_client.shutdown().await?;

        Ok(())
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
    Bulk(Vec<TceCommands>),

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
