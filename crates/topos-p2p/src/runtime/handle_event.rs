use std::io;

use libp2p::{
    core::either::EitherError,
    multiaddr::Protocol,
    swarm::{ConnectionHandlerUpgrErr, SwarmEvent},
};
use tracing::{error, info, warn};

use crate::{event::ComposedEvent, Event, Runtime};

mod discovery;
mod peer_info;
mod transmission;

#[async_trait::async_trait]
pub(crate) trait EventHandler<T> {
    async fn handle(&mut self, event: T);
}

#[async_trait::async_trait]
impl EventHandler<Event> for Runtime {
    async fn handle(&mut self, event: Event) {
        if let Err(error) = self.event_sender.try_send(event) {
            warn!(reason = %error, "Unable to send NetworkEvent event to outer stream");
        }
    }
}

#[async_trait::async_trait]
impl EventHandler<ComposedEvent> for Runtime {
    async fn handle(&mut self, event: ComposedEvent) {
        match event {
            ComposedEvent::Kademlia(event) => self.handle(event).await,
            ComposedEvent::PeerInfo(event) => self.handle(event).await,
            ComposedEvent::Transmission(event) => self.handle(event).await,
            ComposedEvent::OutEvent(event) => self.handle(event).await,
        }
    }
}

#[async_trait::async_trait]
impl
    EventHandler<
        SwarmEvent<
            ComposedEvent,
            EitherError<EitherError<io::Error, io::Error>, ConnectionHandlerUpgrErr<io::Error>>,
        >,
    > for Runtime
{
    async fn handle(
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

            SwarmEvent::Behaviour(event) => {
                self.handle(event).await;
            }

            event => error!("Unhandled event: {event:?}"),
        }
    }
}
