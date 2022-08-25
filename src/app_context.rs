//!
//! Application logic glue
//!
use futures::future::BoxFuture;
use futures::{future::join_all, Stream, StreamExt};
use libp2p::PeerId;
use log::info;
use log::warn;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::Duration;
use tce_trbp::uci::{Certificate, CertificateId};
use tokio::time;
use tokio::{spawn, sync::oneshot};
use topos_p2p::{Client, Event};

use tce_api::{ApiRequests, ApiWorker};

use tce_transport::{TrbpCommands, TrbpEvents};
use tce_trbp::{Errors, ReliableBroadcastClient};

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
        let mut publish = tokio::time::interval(Duration::from_secs(1));

        let mut peers: Vec<PeerId> = Vec::new();

        loop {
            tokio::select! {
                _ = publish.tick() => {
                    if !peers.is_empty() {
                        self.on_net_event(Event::PeersChanged { new_peers: peers.clone() }).await;
                    }
                }
                // API
                Ok(req) = self.api_worker.next_request() => {
                    log::info!("api_worker.next_request(): {:?}", &req);
                    self.on_api_request(req);
                }

                // protocol
                Some(evt) = trb_stream.next() => {
                    log::info!("trbp_cli.next_event(): {:?}", &evt);
                    self.on_protocol_event(evt).await;
                },

                // network
                Some(net_evt) = network_stream.next() => {
                    log::info!("network_stream.next_event(): {:?}", &net_evt);
                    if let Event::PeersChanged {new_peers} = &net_evt {
                        peers = new_peers.clone();
                    }
                    self.on_net_event(net_evt).await;
                }
            }
        }
    }

    fn on_api_request(&mut self, req: ApiRequests) {
        match req {
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
            TrbpEvents::NeedPeers => {
                //todo - launch kademlia query or get latest known?
            }
            TrbpEvents::Broadcast { .. } => {
                //todo ?
            }
            TrbpEvents::EchoSubscribeReq { peers } => {
                let data: Vec<u8> = NetworkMessage::from(TrbpCommands::OnEchoSubscribeReq {
                    from_peer: self.network_client.local_peer_id.to_base58(),
                })
                .into();
                let future_pool = peers
                    .iter()
                    .map(|peer_id| {
                        warn!("Sending echo subscribe");
                        self.network_client.send_request::<_, NetworkMessage>(
                            PeerId::from_str(&peer_id).expect("correct peer_id"),
                            data.clone(),
                        )
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    let r = join_all(future_pool).await;

                    warn!("Receive response {r:?}");
                });
            }
            TrbpEvents::ReadySubscribeReq { peers } => {
                // let data: Vec<u8> = NetworkMessage::from(TrbpCommands::OnReadySubscribeReq {
                //     from_peer: self.network_client.local_peer_id.to_base58(),
                // })
                // .into();
                //
                // let future_pool = peers
                //     .iter()
                //     .map(|peer_id| {
                //         info!("Sending ready subscribe");
                //         self.network_client.send_request::<_, NetworkMessage>(
                //             PeerId::from_str(&peer_id).expect("correct peer_id"),
                //             data.clone(),
                //         )
                //     })
                //     .collect::<Vec<_>>();
                //
                // spawn(async move {
                //     join_all(future_pool).await;
                // });
            }
            TrbpEvents::EchoSubscribeOk { to_peer } => {
                // let to_peer_id = PeerId::from_str(to_peer.as_str()).expect("correct peer_id");
                // let data = NetworkMessage::from(TrbpCommands::OnEchoSubscribeOk {
                //     from_peer: self.network_client.local_peer_id.to_base58(),
                // });
                //
                // info!("Sending echo subscribeOK");
                // spawn(
                //     self.network_client
                //         .send_request::<_, NetworkMessage>(to_peer_id, data),
                // );
            }
            TrbpEvents::ReadySubscribeOk { to_peer } => {
                // let to_peer_id = PeerId::from_str(to_peer.as_str()).expect("correct peer_id");
                // let data = NetworkMessage::from(TrbpCommands::OnReadySubscribeOk {
                //     from_peer: self.network_client.local_peer_id.to_base58(),
                // });
                //
                // info!("Sending ready subscribeOK");
                // spawn(
                //     self.network_client
                //         .send_request::<_, NetworkMessage>(to_peer_id, data),
                // );
            }
            TrbpEvents::Gossip {
                peers,
                cert,
                digest,
            } => {
                // let data: Vec<u8> =
                //     NetworkMessage::from(TrbpCommands::OnGossip { cert, digest }).into();
                //
                // let future_pool = peers
                //     .iter()
                //     .map(|peer_id| {
                //         info!("sending gossip");
                //         self.network_client.send_request::<_, NetworkMessage>(
                //             PeerId::from_str(&peer_id).expect("correct peer_id"),
                //             data.clone(),
                //         )
                //     })
                //     .collect::<Vec<_>>();
                //
                // spawn(async move {
                //     join_all(future_pool).await;
                // });
            }
            TrbpEvents::Echo { peers, cert } => {
                // let data: Vec<u8> = NetworkMessage::from(TrbpCommands::OnEcho {
                //     from_peer: self.network_client.local_peer_id.to_base58(),
                //     cert,
                // })
                // .into();
                //
                // let future_pool = peers
                //     .iter()
                //     .map(|peer_id| {
                //         self.network_client.send_request::<_, NetworkMessage>(
                //             PeerId::from_str(&peer_id).expect("correct peer_id"),
                //             data.clone(),
                //         )
                //     })
                //     .collect::<Vec<_>>();
                //
                // spawn(async move {
                //     join_all(future_pool).await;
                // });
            }
            TrbpEvents::Ready { peers, cert } => {
                // let data: Vec<u8> = NetworkMessage::from(TrbpCommands::OnReady {
                //     from_peer: self.network_client.local_peer_id.to_base58(),
                //     cert,
                // })
                // .into();
                // let future_pool = peers
                //     .iter()
                //     .map(|peer_id| {
                //         self.network_client.send_request::<_, NetworkMessage>(
                //             PeerId::from_str(&peer_id).expect("correct peer_id"),
                //             data.clone(),
                //         )
                //     })
                //     .collect::<Vec<_>>();
                //
                // spawn(async move {
                //     join_all(future_pool).await;
                // });
            }
            evt => {
                log::debug!("Unhandled event: {:?}", evt);
            }
        }
    }

    async fn on_net_event(&mut self, evt: Event) {
        log::debug!("on_net_event ({:?}", &evt);
        match evt {
            Event::PeersChanged { new_peers } => {
                let data: Vec<u8> = NetworkMessage::from(TrbpCommands::OnEchoSubscribeReq {
                    from_peer: self.network_client.local_peer_id.to_base58(),
                })
                .into();

                let future_pool = new_peers
                    .iter()
                    .map(|peer_id| {
                        warn!("Send EchoSubscribe");
                        self.network_client
                            .send_request::<_, NetworkMessage>(*peer_id, data.clone())
                    })
                    .collect::<Vec<_>>();

                spawn(async move {
                    let r = join_all(future_pool).await;
                    warn!("Get EchoSubscribe Response {r:?}");
                });
            }
            Event::TransmissionOnReq {
                from: _,
                data,
                channel,
                ..
            } => {
                let msg: NetworkMessage = data.into();
                match msg {
                    NetworkMessage::Cmd(cmd) => {
                        //  redirect
                        // let _ = self.trbp_cli.eval(cmd);

                        spawn(self.network_client.respond_to_request(vec![], channel));
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
