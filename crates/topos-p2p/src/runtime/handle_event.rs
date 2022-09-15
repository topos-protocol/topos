use libp2p::{
    core::{either::EitherError, Endpoint},
    multiaddr::Protocol,
    ping::PingFailure as Failure,
    swarm::{ConnectionHandlerUpgrErr, SwarmEvent},
};
use std::io;
use tracing::{error, info, instrument, warn};

use crate::{event::ComposedEvent, Runtime};

impl Runtime {
    #[instrument(name = "Runtime::handle_event", skip_all, fields(peer_id = %self.local_peer_id))]
    pub(crate) async fn handle_event(
        &mut self,
        event: SwarmEvent<
            ComposedEvent,
            // EitherError<EitherError<io::Error, io::Error>, ConnectionHandlerUpgrErr<io::Error>>,
            EitherError<
                EitherError<EitherError<Failure, io::Error>, io::Error>,
                ConnectionHandlerUpgrErr<io::Error>,
            >,
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
                    peer_id = peer_id.to_string(),
                    direction = format!("{:?}", endpoint.to_endpoint()),
                    addresses = endpoint.get_remote_address().to_string(),
                    "Connection established with peer: {peer_id}"
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

            SwarmEvent::ConnectionClosed {
                peer_id, endpoint, ..
            } => {
                info!(
                    peer_id = peer_id.to_string(),
                    direction = match endpoint.to_endpoint() {
                        Endpoint::Dialer => "dialer",
                        Endpoint::Listener => "listener",
                    },
                    addresses = endpoint.get_remote_address().to_string(),
                    "Connection closed with peer {}",
                    peer_id
                );
            }

            SwarmEvent::Dialing(peer_id) => {
                info!("Dial {:?} from {:?}", peer_id, *self.swarm.local_peer_id());
            }

            SwarmEvent::Behaviour(ComposedEvent::OutEvent(event)) => {
                if let Err(error) = self.event_sender.try_send(event) {
                    warn!(reason = %error, "Unable to send NetworkEvent event to outer stream");
                }
            }
            event => error!("Unhandled event: {event:?}"),
        }
    }
}
