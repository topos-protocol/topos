use std::borrow::Cow;
use std::io;

use libp2p::core::ProtocolName;
use libp2p::request_response::{RequestResponseEvent, RequestResponseMessage};
use libp2p::{
    core::either::EitherError,
    identify::Info as IdentifyInfo,
    kad::{record::Key, Quorum, Record},
    multiaddr::Protocol,
    swarm::{ConnectionHandlerUpgrErr, SwarmEvent},
};
use tracing::{error, info, instrument, warn};

use crate::{
    behaviour::{
        discovery::DiscoveryOut, peer_info::PeerInfoOut,
        transmission::protocol::TransmissionProtocol,
    },
    event::ComposedEvent,
    Event, Runtime,
};

impl Runtime {
    #[instrument(name = "Runtime::handle_event", skip_all, fields(peer_id = %self.local_peer_id))]
    pub(crate) async fn handle_event(
        &mut self,
        event: SwarmEvent<
            ComposedEvent,
            EitherError<EitherError<io::Error, io::Error>, ConnectionHandlerUpgrErr<io::Error>>,
        >,
    ) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!(
                    "Local node is listening on {:?}",
                    address.with(Protocol::P2p(self.local_peer_id.into())),
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(_peer_id) = peer_id {
                    error!("OutgoingConnectionError {error:?}");
                }
            }

            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!(
                    "Connection established with peer {peer_id} as {:?}",
                    endpoint.to_endpoint()
                );
            }
            SwarmEvent::IncomingConnection { local_addr, .. } => {
                info!("IncomingConnection {local_addr}")
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                info!(
                    "ListenerClosed {:?}: listener_id{listener_id:?} | addresses: {addresses:?} | reason: {reason:?}",
                    *self.swarm.local_peer_id()
                );
            }

            SwarmEvent::ConnectionClosed { peer_id, .. } => info!("ConnectionClosed {peer_id}"),

            SwarmEvent::Dialing(peer_id) => {
                info!("Dial {:?} from {:?}", peer_id, *self.swarm.local_peer_id());
            }

            SwarmEvent::Behaviour(ComposedEvent::OutEvent(event)) => {
                if let Err(error) = self.event_sender.try_send(event) {
                    warn!(reason = %error, "Unable to send NetworkEvent event to outer stream");
                }
            }

            SwarmEvent::Behaviour(ComposedEvent::Discovery(event)) => match event {
                DiscoveryOut::SelfDiscovered(peer, addr) => {
                    self.swarm
                        .behaviour_mut()
                        .transmission
                        .inner
                        .add_address(&peer, addr);
                }
                DiscoveryOut::Discovered(peer) if self.bootstrapped => {
                    if self.swarm.behaviour_mut().discovery.peers.insert(peer) {
                        _ = self.event_sender.try_send(Event::PeersChanged {
                            new_peers: vec![peer],
                        });
                    }
                }
                DiscoveryOut::UnroutablePeer(peer) if self.bootstrapped => {
                    if self.swarm.behaviour_mut().discovery.peers.remove(&peer) {
                        let peers = self
                            .swarm
                            .behaviour_mut()
                            .discovery
                            .peers
                            .iter()
                            .cloned()
                            .collect();
                        _ = self
                            .event_sender
                            .try_send(Event::PeersChanged { new_peers: peers });
                    }
                }

                DiscoveryOut::BootstrapOk => {
                    info!("Bootstrapping finished");
                    let key = Key::new(&self.local_peer_id.to_string());
                    _ = self
                        .swarm
                        .behaviour_mut()
                        .discovery
                        .kademlia
                        .put_record(Record::new(key, self.addresses.to_vec()), Quorum::All);
                }
                _ => {}
            },
            SwarmEvent::Behaviour(ComposedEvent::PeerInfo(event)) => match event {
                PeerInfoOut::Identified { peer_id, info } => {
                    let IdentifyInfo {
                        protocol_version,
                        listen_addrs,
                        protocols,
                        ..
                    } = *info;

                    if protocol_version.as_bytes() == TransmissionProtocol().protocol_name()
                        && protocols.iter().any(|p| {
                            self.swarm
                                .behaviour()
                                .discovery
                                .kademlia
                                .protocol_names()
                                .contains(&Cow::Borrowed(p.as_bytes()))
                        })
                    {
                        for addr in listen_addrs {
                            self.swarm
                                .behaviour_mut()
                                .transmission
                                .inner
                                .add_address(&peer_id, addr.clone());
                            self.swarm
                                .behaviour_mut()
                                .discovery
                                .kademlia
                                .add_address(&peer_id, addr);
                        }
                    }
                }

                PeerInfoOut::Disconnected { peer_id } => {
                    if self.swarm.behaviour_mut().discovery.peers.remove(&peer_id) {
                        _ = self
                            .event_sender
                            .try_send(Event::PeerDisconnected { peer_id });

                        let peers = self
                            .swarm
                            .behaviour_mut()
                            .discovery
                            .peers
                            .iter()
                            .cloned()
                            .collect();
                        _ = self
                            .event_sender
                            .try_send(Event::PeersChanged { new_peers: peers });
                    }
                }
            },

            SwarmEvent::Behaviour(ComposedEvent::Transmission(RequestResponseEvent::Message {
                peer,
                message:
                    RequestResponseMessage::Request {
                        request, channel, ..
                    },
            })) => {
                _ = self.event_sender.try_send(Event::TransmissionOnReq {
                    from: peer,
                    data: request.0,
                    channel,
                });
            }
            event => error!("Unhandled event: {event:?}"),
        }
    }
}
