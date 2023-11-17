use libp2p::{
    multiaddr::Protocol,
    swarm::{derive_prelude::Either, SwarmEvent},
};
use tracing::{debug, error, info, warn};

use crate::{event::ComposedEvent, Event, Runtime};

mod discovery;
mod gossipsub;
mod grpc;
mod peer_info;

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
            ComposedEvent::Gossipsub(event) => self.handle(event).await,
            ComposedEvent::Grpc(event) => self.handle(event).await,
            ComposedEvent::Void => (),
        }
    }
}

#[async_trait::async_trait]
impl
    EventHandler<
        SwarmEvent<
            ComposedEvent,
            Either<Either<Either<std::io::Error, std::io::Error>, void::Void>, void::Void>,
        >,
    > for Runtime
{
    async fn handle(
        &mut self,
        event: SwarmEvent<
            ComposedEvent,
            Either<Either<Either<std::io::Error, std::io::Error>, void::Void>, void::Void>,
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
                    address.with(Protocol::P2p(self.local_peer_id)),
                );

                self.active_listeners.insert(listener_id);
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

            incoming_connection_error @ SwarmEvent::IncomingConnectionError { .. } => {
                error!("{:?}", incoming_connection_error);
            }

            SwarmEvent::IncomingConnection { local_addr, .. } => {
                info!("IncomingConnection {local_addr}")
            }
            SwarmEvent::ListenerClosed {
                listener_id,
                addresses,
                reason,
            } => {
                debug!(
                    "ListenerClosed {:?}: listener_id{listener_id:?} | addresses: {addresses:?} | \
                     reason: {reason:?}",
                    *self.swarm.local_peer_id()
                );
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                debug!("ConnectionClosed {peer_id} because of {cause:?}");
            }

            SwarmEvent::Dialing { peer_id, .. } => {}

            SwarmEvent::Behaviour(event) => {
                self.handle(event).await;
            }

            event => error!("Unhandled event: {event:?}"),
        }
    }
}
