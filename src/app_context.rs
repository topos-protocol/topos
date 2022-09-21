//!
//! Application logic glue
//!
use futures::{future::join_all, Stream, StreamExt};
use libp2p::PeerId;
use log::error;
use log::info;
use log::warn;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use topos_tce_api::RuntimeClient as ApiClient;
use topos_tce_api::RuntimeEvent as ApiEvent;
// use tce_api::web_api::PeerChanged;
// use tce_api::{ApiRequests, ApiWorker};
use tce_transport::{TrbpCommands, TrbpEvents};
use tokio::spawn;
use tokio::sync::oneshot;
use topos_p2p::{Client as NetworkClient, Event as NetEvent};
use topos_tce_broadcast::sampler::SampleType;
use topos_tce_broadcast::{ReliableBroadcastClient, SamplerCommand};

use crate::storage::Storage;

/// Top-level transducer main app context & driver (alike)
///
/// Implements <...Host> traits for network and Api, listens for protocol events in events
/// (store is not active component).
///
/// In the end we shall come to design where this struct receives
/// config+data as input and runs app returning data as output
///
pub struct AppContext {
    pub trbp_cli: ReliableBroadcastClient,
    pub network_client: NetworkClient,
    pub api_client: ApiClient,
    pub pending_storage: Box<dyn Storage>,
}

impl AppContext {
    /// Factory
    pub fn new(
        pending_storage: impl Storage,
        trbp_cli: ReliableBroadcastClient,
        network_client: NetworkClient,
        api_client: ApiClient,
    ) -> Self {
        Self {
            trbp_cli,
            network_client,
            api_client,
            pending_storage: Box::new(pending_storage),
        }
    }

    /// Main processing loop
    pub async fn run(
        mut self,
        mut network_stream: impl Stream<Item = NetEvent> + Unpin,
        mut trb_stream: impl Stream<Item = Result<TrbpEvents, ()>> + Unpin,
        mut api_stream: impl Stream<Item = ApiEvent> + Unpin,
    ) {
        loop {
            tokio::select! {

                // protocol
                Some(Ok(evt)) = trb_stream.next() => {
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

            }
        }
    }

    async fn on_api_event(&mut self, event: ApiEvent) {
        match event {
            ApiEvent::CertificateSubmitted {
                certificate,
                sender,
            } => {
                _ = self.pending_storage.persist(certificate.clone()).await;
                spawn(self.trbp_cli.broadcast_new_certificate(certificate));
                _ = sender.send(Ok(()));
            }
        }
    }

    async fn on_protocol_event(&mut self, evt: TrbpEvents) {
        log::debug!("on_protocol_event : {:?}", &evt);
        match evt {
            TrbpEvents::CertificateDelivered { certificate } => {
                _ = self.pending_storage.remove(&certificate.cert_id).await;
                spawn(self.api_client.dispatch_certificate(certificate));
            }

            TrbpEvents::EchoSubscribeReq { peers } => {
                let data: Vec<u8> = NetworkMessage::from(TrbpCommands::OnEchoSubscribeReq {
                    from_peer: self.network_client.local_peer_id.to_base58(),
                })
                .into();

                let command_sender = self.trbp_cli.get_sampler_channel();

                let future_pool = peers
                    .iter()
                    .map(|peer_id| {
                        warn!("Sending echo subscribe");
                        self.network_client.send_request::<_, NetworkMessage>(
                            PeerId::from_str(peer_id).expect("correct peer_id"),
                            data.clone(),
                        )
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    let results = join_all(future_pool).await;

                    for result in results {
                        match result {
                            Ok(message) => match message {
                                NetworkMessage::Cmd(TrbpCommands::OnEchoSubscribeOk {
                                    from_peer,
                                }) => {
                                    info!("Receive response to EchoSubscribe",);
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
                                    error!("Receive an unexpected message as a response {msg:?}")
                                }
                            },
                            Err(error) => {
                                error!("An error occured when sending EchoSubscribe {error:?}")
                            }
                        }
                    }
                });
            }
            TrbpEvents::ReadySubscribeReq { peers } => {
                let data: Vec<u8> = NetworkMessage::from(TrbpCommands::OnReadySubscribeReq {
                    from_peer: self.network_client.local_peer_id.to_base58(),
                })
                .into();

                let command_sender = self.trbp_cli.get_sampler_channel();

                let future_pool = peers
                    .iter()
                    .map(|peer_id| {
                        info!("Sending ready subscribe");
                        self.network_client.send_request::<_, NetworkMessage>(
                            PeerId::from_str(peer_id).expect("correct peer_id"),
                            data.clone(),
                        )
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    let results = join_all(future_pool).await;

                    for result in results {
                        match result {
                            Ok(message) => match message {
                                NetworkMessage::Cmd(TrbpCommands::OnReadySubscribeOk {
                                    from_peer,
                                }) => {
                                    info!("Receive response to ReadySubscribe");
                                    let (sender_ready, receiver_ready) = oneshot::channel();
                                    let _ = command_sender
                                        .send(SamplerCommand::ConfirmPeer {
                                            peer: from_peer.clone(),
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
                                    error!("Receive an unexpected message as a response {msg:?}")
                                }
                            },
                            Err(error) => {
                                error!("An error occured when sending ReadySubscribe {error:?}")
                            }
                        }
                    }
                });
            }

            evt => {
                log::debug!("Unhandled event: {:?}", evt);
            }
        }
    }

    async fn on_net_event(&mut self, evt: NetEvent) {
        match evt {
            NetEvent::PeersChanged { .. } => {}

            NetEvent::TransmissionOnReq {
                from: _,
                data,
                channel,
                ..
            } => {
                let my_peer = self.network_client.local_peer_id;
                let msg: NetworkMessage = data.into();
                match msg {
                    NetworkMessage::Cmd(cmd) => {
                        info!("Receive TransmissionOnReq {:?}", cmd);

                        match cmd {
                            TrbpCommands::OnEchoSubscribeReq { from_peer } => {
                                self.trbp_cli
                                    .add_confirmed_peer_to_sample(
                                        SampleType::EchoSubscriber,
                                        from_peer,
                                    )
                                    .await;

                                spawn(self.network_client.respond_to_request(
                                    NetworkMessage::from(TrbpCommands::OnEchoSubscribeOk {
                                        from_peer: my_peer.to_base58(),
                                    }),
                                    channel,
                                ));
                            }

                            TrbpCommands::OnReadySubscribeReq { from_peer } => {
                                self.trbp_cli
                                    .add_confirmed_peer_to_sample(
                                        SampleType::ReadySubscriber,
                                        from_peer,
                                    )
                                    .await;
                                spawn(self.network_client.respond_to_request(
                                    NetworkMessage::from(TrbpCommands::OnReadySubscribeOk {
                                        from_peer: my_peer.to_base58(),
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
enum NetworkMessage {
    Cmd(TrbpCommands),
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
impl From<TrbpCommands> for NetworkMessage {
    fn from(cmd: TrbpCommands) -> Self {
        Self::Cmd(cmd)
    }
}
