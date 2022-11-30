use std::borrow::Cow;
use std::io;

use libp2p::core::ProtocolName;
use libp2p::kad::{KademliaEvent, QueryResult};
use libp2p::request_response::{RequestResponseEvent, RequestResponseMessage};
use libp2p::Multiaddr;
use libp2p::{
    core::either::EitherError,
    identify::Event as IdentifyEvent,
    identify::Info as IdentifyInfo,
    kad::{record::Key, Quorum, Record},
    multiaddr::Protocol,
    swarm::{ConnectionHandlerUpgrErr, SwarmEvent},
};
use tracing::{error, info, instrument, warn};

use crate::{
    behaviour::transmission::protocol::TransmissionProtocol, event::ComposedEvent, Event, Runtime,
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
            SwarmEvent::NewListenAddr {
                listener_id,
                address,
                ..
            } => {
                info!(
                    "Local node is listening on {:?}",
                    address.with(Protocol::P2p(self.local_peer_id.into())),
                );

                self.active_listeners.insert(listener_id);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(_peer_id) = peer_id {
                    error!("OutgoingConnectionError {error:?}");
                }

                error!("Dial failure: {error:?}");
                if let Some(peer_id) = peer_id {
                    if let Some(sender) = self.pending_dial.remove(&peer_id) {
                        if sender.send(Err(crate::error::P2PError::DialError)).is_err() {
                            warn!("Could not notify dial failure because initiator is dropped");
                        }
                    }
                }
            }

            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!(
                    "Connection established with peer {peer_id} as {:?}",
                    endpoint.to_endpoint()
                );
                if let Some(sender) = self.pending_dial.remove(&peer_id) {
                    self.peers.insert(peer_id);
                    if sender.send(Ok(())).is_err() {
                        warn!(
                            %peer_id,
                            "Could not notify successful dial with {peer_id}: initiator dropped"
                        );
                    }
                }
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

            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                info!("ConnectionClosed {peer_id}");
                if self.peers.remove(&peer_id) {
                    _ = self
                        .event_sender
                        .try_send(Event::PeerDisconnected { peer_id });

                    let peers = self.peers.iter().cloned().collect();

                    _ = self
                        .event_sender
                        .try_send(Event::PeersChanged { new_peers: peers });
                }
            }

            SwarmEvent::Dialing(peer_id) => {
                info!("Dial {:?} from {:?}", peer_id, *self.swarm.local_peer_id());
            }

            SwarmEvent::Behaviour(ComposedEvent::OutEvent(event)) => {
                if let Err(error) = self.event_sender.try_send(event) {
                    warn!(reason = %error, "Unable to send NetworkEvent event to outer stream");
                }
            }

            SwarmEvent::Behaviour(ComposedEvent::PeerInfo(event)) => {
                if let IdentifyEvent::Received { peer_id, info, .. } = *event {
                    let IdentifyInfo {
                        protocol_version,
                        listen_addrs,
                        protocols,
                        ..
                    } = info;

                    if protocol_version.as_bytes() == TransmissionProtocol().protocol_name()
                        && protocols.iter().any(|p| {
                            self.swarm
                                .behaviour()
                                .discovery
                                .protocol_names()
                                .contains(&Cow::Borrowed(p.as_bytes()))
                        })
                    {
                        for addr in listen_addrs {
                            self.swarm
                                .behaviour_mut()
                                .transmission
                                .add_address(&peer_id, addr.clone());
                            self.swarm
                                .behaviour_mut()
                                .discovery
                                .add_address(&peer_id, addr);
                        }
                    }
                }
            }

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

            SwarmEvent::Behaviour(ComposedEvent::Transmission(RequestResponseEvent::Message {
                message:
                    RequestResponseMessage::Response {
                        request_id,
                        response,
                    },
                ..
            })) => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    if sender.send(Ok(response.0)).is_err() {
                        warn!("Could not send response to request {request_id} because initiator is dropped");
                    }
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Transmission(
                RequestResponseEvent::OutboundFailure {
                    request_id, error, ..
                },
            )) => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    if sender.send(Err(error.into())).is_err() {
                        warn!("Could not send RequestFailure for request {request_id} because initiator is dropped");
                    }
                } else {
                    warn!("Received an OutboundRequest failure for an unknown request {request_id}")
                }
            }

            SwarmEvent::Behaviour(ComposedEvent::Kademlia(event)) => match event {
                KademliaEvent::RoutingUpdated {
                    peer,
                    is_new_peer: true,
                    ..
                } if self.bootstrapped => {
                    if self.peers.insert(peer) {
                        let peers = self.peers.iter().cloned().collect();
                        _ = self
                            .event_sender
                            .try_send(Event::PeersChanged { new_peers: peers });
                    }
                }
                KademliaEvent::UnroutablePeer { peer } => {
                    if self.peers.remove(&peer) {
                        let peers = self.peers.iter().cloned().collect();
                        _ = self
                            .event_sender
                            .try_send(Event::PeersChanged { new_peers: peers });
                    }
                }
                KademliaEvent::OutboundQueryCompleted {
                    result: QueryResult::Bootstrap(res),
                    ..
                } => match res {
                    Ok(_) => {
                        info!("Bootstrapping finished");
                        let key = Key::new(&self.local_peer_id.to_string());
                        _ = self
                            .swarm
                            .behaviour_mut()
                            .discovery
                            .put_record(Record::new(key, self.addresses.to_vec()), Quorum::All);
                    }
                    Err(e) => error!("Error: bootstrap : {e:?}"),
                },
                KademliaEvent::OutboundQueryCompleted {
                    result: QueryResult::PutRecord(Err(e)),
                    ..
                } => {
                    error!("{e:?}");
                }
                KademliaEvent::OutboundQueryCompleted {
                    result: QueryResult::GetRecord(res),
                    id,
                    ..
                } => match res {
                    Ok(result) => {
                        if let Some(sender) = self.pending_record_requests.remove(&id) {
                            if let Some(peer_record) = result.records.first() {
                                if let Ok(addr) =
                                    Multiaddr::try_from(peer_record.record.value.clone())
                                {
                                    if let Some(peer_id) = peer_record.record.publisher {
                                        if !sender.is_closed() {
                                            self.swarm
                                                .behaviour_mut()
                                                .discovery
                                                .add_address(&peer_id, addr.clone());

                                            if sender.send(Ok(vec![addr.clone()])).is_err() {
                                                // TODO: Hash the QueryId
                                                warn!("Could not notify Record query ({id:?}) response because initiator is dropped");
                                            }
                                        }
                                        self.swarm
                                            .behaviour_mut()
                                            .transmission
                                            .add_address(&peer_id, addr);
                                    }
                                }
                            }
                        }
                    }

                    Err(error) => error!("{error:?}"),
                },
                _ => {}
            },

            event => error!("Unhandled event: {event:?}"),
        }
    }
}
