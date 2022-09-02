use std::io;

use libp2p::{
    core::either::EitherError,
    multiaddr::Protocol,
    swarm::{ConnectionHandlerUpgrErr, SwarmEvent},
};
use tracing::{error, info};

use crate::{event::ComposedEvent, Runtime};

impl Runtime {
    pub(crate) async fn handle_event(
        &mut self,
        event: SwarmEvent<
            ComposedEvent,
            EitherError<EitherError<io::Error, io::Error>, ConnectionHandlerUpgrErr<io::Error>>,
        >,
    ) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                let local_peer_id = *self.swarm.local_peer_id();
                info!(
                    "Local node is listening on {:?}, peer_id: {:?}",
                    address.with(Protocol::P2p(local_peer_id.into())),
                    local_peer_id
                );
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(_peer_id) = peer_id {
                    error!("OutgoingConnectionError {error:?}");
                }
            }

            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connection established with peer: {peer_id}");
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
                self.event_sender
                    .try_send(event)
                    .expect("Cannot send event to stream");
            }
            event => error!("Unhandled event: {event:?}"),
        }
    }
}
