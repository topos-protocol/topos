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
use tce_api::web_api::PeerChanged;
use tce_api::{ApiRequests, ApiWorker};
use tce_transport::{TrbpCommands, TrbpEvents};
use tce_trbp::sampler::SampleType;
use tce_trbp::{ReliableBroadcastClient, SamplerCommand, TrbInternalCommand};
use tokio::spawn;
use topos_p2p::{Client, Event};

/// Top-level transducer main app context & driver (alike)
///
/// Implements <...Host> traits for network and Api, listens for protocol events in events
/// (store is not active component).
///
/// In the end we shall come to design where this struct receives
/// config+data as input and runs app returning data as output
///
pub struct AppContext {
    //pub store: Box<dyn trb_store::TrbStore>,
    pub trbp_cli: ReliableBroadcastClient,
    pub api_worker: ApiWorker,
    pub network_client: Client,
}

impl AppContext {
    /// Factory
    pub fn new(
        //store: Box<dyn trb_store::TrbStore>,
        trbp_cli: ReliableBroadcastClient,
        api_worker: ApiWorker,
        network_client: Client,
    ) -> Self {
        Self {
            //store,
            trbp_cli,
            api_worker,
            network_client,
        }
    }

    /// Main processing loop
    pub async fn run(
        mut self,
        mut network_stream: impl Stream<Item = topos_p2p::Event> + Unpin,
        mut trb_stream: impl Stream<Item = TrbpEvents> + Unpin,
    ) {
        loop {
            tokio::select! {

                // API
                Ok(req) = self.api_worker.next_request() => {
                    self.on_api_request(req);
                }

                // protocol
                Some(evt) = trb_stream.next() => {
                    self.on_protocol_event(evt).await;
                },

                // network
                Some(net_evt) = network_stream.next() => {
                    self.on_net_event(net_evt).await;
                }
            }
        }
    }

    fn on_api_request(&mut self, req: ApiRequests) {
        match req {
            ApiRequests::PeerChanged {
                req: PeerChanged { mut peers },
                resp_channel,
            } => {
                resp_channel.send(()).expect("sync send");
                info!("Peers have changed, notify the sampler");
                self.trbp_cli.peer_changed(
                    peers
                        .iter_mut()
                        .map(|e| e.parse::<PeerId>().unwrap().to_base58())
                        .collect(),
                );
            }

            ApiRequests::SubmitCert { req, resp_channel } => {
                self.trbp_cli
                    .eval(TrbpCommands::OnBroadcast { cert: req.cert })
                    .expect("SubmitCert");
                resp_channel.send(()).expect("sync send");
            }

            ApiRequests::DeliveredCerts { req, resp_channel } => {
                let future = self
                    .trbp_cli
                    .delivered_certs(req.subnet_id, req.from_cert_id);

                spawn(async move {
                    match future.await {
                        Ok(the_certs) => {
                            resp_channel.send(the_certs).expect("sync send");
                        }

                        _ => {
                            log::error!("Request failure {:?}", req);
                        }
                    }
                });
            }
        }
    }

    async fn on_protocol_event(&mut self, evt: TrbpEvents) {
        log::debug!("on_protocol_event : {:?}", &evt);
        match evt {
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
                    let r = join_all(future_pool).await;

                    r.into_iter().for_each(|result| match result {
                        Ok(message) => match message {
                            NetworkMessage::Cmd(TrbpCommands::OnEchoSubscribeOk { from_peer }) => {
                                info!("Receive response to EchoSubscribe",);
                                let _ = command_sender.send(TrbInternalCommand::Sampler(
                                    SamplerCommand::ConfirmPeer {
                                        peer: from_peer,
                                        sample_type: SampleType::EchoSubscription,
                                    },
                                ));
                            }
                            msg => error!("Receive an unexpected message as a response {msg:?}"),
                        },
                        Err(error) => {
                            error!("An error occured when sending EchoSubscribe {error:?}")
                        }
                    });
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
                    let r = join_all(future_pool).await;

                    r.into_iter().for_each(|result| match result {
                        Ok(message) => match message {
                            NetworkMessage::Cmd(TrbpCommands::OnReadySubscribeOk { from_peer }) => {
                                info!("Receive response to ReadySubscribe");
                                let _ = command_sender.send(TrbInternalCommand::Sampler(
                                    SamplerCommand::ConfirmPeer {
                                        peer: from_peer.clone(),
                                        sample_type: SampleType::ReadySubscription,
                                    },
                                ));
                                let _ = command_sender.send(TrbInternalCommand::Sampler(
                                    SamplerCommand::ConfirmPeer {
                                        peer: from_peer,
                                        sample_type: SampleType::DeliverySubscription,
                                    },
                                ));
                            }
                            msg => error!("Receive an unexpected message as a response {msg:?}"),
                        },
                        Err(error) => {
                            error!("An error occured when sending ReadySubscribe {error:?}")
                        }
                    });
                });
            }

            evt => {
                log::debug!("Unhandled event: {:?}", evt);
            }
        }
    }

    async fn on_net_event(&mut self, evt: Event) {
        match evt {
            Event::PeersChanged { .. } => {}

            Event::TransmissionOnReq {
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
                                self.trbp_cli.add_confirmed_peer_to_sample(
                                    SampleType::EchoSubscriber,
                                    from_peer,
                                );
                                spawn(self.network_client.respond_to_request(
                                    NetworkMessage::from(TrbpCommands::OnEchoSubscribeOk {
                                        from_peer: my_peer.to_base58(),
                                    }),
                                    channel,
                                ));
                            }

                            TrbpCommands::OnReadySubscribeReq { from_peer } => {
                                self.trbp_cli.add_confirmed_peer_to_sample(
                                    SampleType::ReadySubscriber,
                                    from_peer,
                                );
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
