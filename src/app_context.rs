//!
//! Application logic glue
//!
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use tce_api::{ApiRequests, ApiWorker};
use tce_net::{NetworkCommands, NetworkEvents, NetworkWorker};

use tce_transport::{TrbpCommands, TrbpEvents};
use tce_trbp::ReliableBroadcastClient;

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
    pub network_worker: NetworkWorker,
}

impl AppContext {
    /// Factory
    pub fn new(
        //store: Box<dyn trb_store::TrbStore>,
        trbp_cli: ReliableBroadcastClient,
        api_worker: ApiWorker,
        network_worker: NetworkWorker,
    ) -> Self {
        Self {
            //store,
            trbp_cli,
            api_worker,
            network_worker,
        }
    }

    /// Main processing loop
    pub(crate) async fn run(&mut self) {
        loop {
            tokio::select! {
                // API
                Ok(req) = self.api_worker.next_request() => {
                    log::debug!("api_worker.next_request(): {:?}", &req);
                    self.on_api_request(req);
                }

                // protocol
                Ok(evt) = self.trbp_cli.next_event() => {
                    log::debug!("trbp_cli.next_event(): {:?}", &evt);
                    self.on_trbp_event(evt).await;
                },

                // network
                Ok(net_evt) = self.network_worker.next_event() => {
                    log::debug!("network_worker.next_event(): {:?}", &net_evt);
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
                match self
                    .trbp_cli
                    .delivered_certs_ids(req.subnet_id, req.from_cert_id)
                {
                    Ok(Some(v)) => {
                        let the_certs = v
                            .iter()
                            .map(|&id| self.trbp_cli.cert_by_id(id).expect("cert").unwrap())
                            .collect();
                        resp_channel.send(the_certs).expect("sync send");
                    }
                    Ok(None) => {
                        log::debug!("No delivered certificate for that subnet");
                        resp_channel.send(Vec::new()).expect("sync send");
                    }
                    _ => {
                        log::error!("Request failure {:?}", req);
                    }
                }
            }
        }
    }

    async fn on_trbp_event(&mut self, evt: TrbpEvents) {
        log::debug!("on_trbp_event : {:?}", &evt);
        match evt {
            TrbpEvents::NeedPeers => {
                //todo
                //  launch kademlia query or get latest known?
            }
            TrbpEvents::Broadcast { .. } => {
                //todo ?
            }
            TrbpEvents::EchoSubscribeReq { peers } => {
                let cmd = NetworkCommands::TransmissionReq {
                    ext_req_id: "".to_string(),
                    to: peers
                        .iter()
                        .map(|e| PeerId::from_str(e.as_str()).expect("correct peer_id"))
                        .collect(),
                    data: NetworkMessage::from(TrbpCommands::OnEchoSubscribeReq {
                        from_peer: self.network_worker.my_peer_id.to_base58(),
                    })
                    .into(),
                };
                self.network_worker.eval(cmd).await;
            }
            TrbpEvents::ReadySubscribeReq { peers } => {
                let cmd = NetworkCommands::TransmissionReq {
                    ext_req_id: "".to_string(),
                    to: peers
                        .iter()
                        .map(|e| PeerId::from_str(e.as_str()).expect("correct peer_id"))
                        .collect(),
                    data: NetworkMessage::from(TrbpCommands::OnReadySubscribeReq {
                        from_peer: self.network_worker.my_peer_id.to_base58(),
                    })
                    .into(),
                };
                self.network_worker.eval(cmd).await;
            }
            TrbpEvents::EchoSubscribeOk { to_peer } => {
                let to_peer_id = PeerId::from_str(to_peer.as_str()).expect("correct peer_id");
                let cmd = NetworkCommands::TransmissionReq {
                    ext_req_id: "".to_string(),
                    to: vec![to_peer_id],
                    data: NetworkMessage::from(TrbpCommands::OnEchoSubscribeOk {
                        from_peer: self.network_worker.my_peer_id.to_base58(),
                    })
                    .into(),
                };
                self.network_worker.eval(cmd).await;
            }
            TrbpEvents::ReadySubscribeOk { to_peer } => {
                let to_peer_id = PeerId::from_str(to_peer.as_str()).expect("correct peer_id");
                let cmd = NetworkCommands::TransmissionReq {
                    ext_req_id: "".to_string(),
                    to: vec![to_peer_id],
                    data: NetworkMessage::from(TrbpCommands::OnReadySubscribeOk {
                        from_peer: self.network_worker.my_peer_id.to_base58(),
                    })
                    .into(),
                };
                self.network_worker.eval(cmd).await;
            }
            TrbpEvents::Gossip {
                peers,
                cert,
                digest,
            } => {
                let cmd = NetworkCommands::TransmissionReq {
                    ext_req_id: "".to_string(),
                    to: peers
                        .iter()
                        .map(|e| PeerId::from_str(e.as_str()).expect("correct peer_id"))
                        .collect(),
                    data: NetworkMessage::from(TrbpCommands::OnGossip { cert, digest }).into(),
                };
                self.network_worker.eval(cmd).await;
            }
            TrbpEvents::Echo { peers, cert } => {
                let cmd = NetworkCommands::TransmissionReq {
                    ext_req_id: "".to_string(),
                    to: peers
                        .iter()
                        .map(|e| PeerId::from_str(e.as_str()).expect("correct peer_id"))
                        .collect(),
                    data: NetworkMessage::from(TrbpCommands::OnEcho {
                        from_peer: self.network_worker.my_peer_id.to_base58(),
                        cert,
                    })
                    .into(),
                };
                self.network_worker.eval(cmd).await;
            }
            TrbpEvents::Ready { peers, cert } => {
                let cmd = NetworkCommands::TransmissionReq {
                    ext_req_id: "".to_string(),
                    to: peers
                        .iter()
                        .map(|e| PeerId::from_str(e.as_str()).expect("correct peer_id"))
                        .collect(),
                    data: NetworkMessage::from(TrbpCommands::OnReady {
                        from_peer: self.network_worker.my_peer_id.to_base58(),
                        cert,
                    })
                    .into(),
                };
                self.network_worker.eval(cmd).await;
            }
            evt => {
                log::debug!("Unhandled event: {:?}", evt);
            }
        }
    }

    async fn on_net_event(&mut self, evt: NetworkEvents) {
        match evt {
            NetworkEvents::KadPeersChanged { new_peers } => {
                //  notify the protocol
                let _ = self.trbp_cli.eval(TrbpCommands::OnVisiblePeersChanged {
                    peers: new_peers.iter().map(|e| e.to_base58()).collect(),
                });
            }
            NetworkEvents::TransmissionOnReq {
                from: _,
                data,
            } => {
                let msg: NetworkMessage = data.into();
                match msg {
                    NetworkMessage::Cmd(cmd) => {
                        //  redirect
                        let _ = self.trbp_cli.eval(cmd);
                    }
                }
            }
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
