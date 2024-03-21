use libp2p::{multiaddr::Protocol, swarm::SwarmEvent};
use tracing::{debug, error, info, warn};

use crate::{error::P2PError, event::ComposedEvent, Event, Runtime};

mod discovery;
mod gossipsub;
mod grpc;
mod peer_info;

pub type EventResult = Result<(), P2PError>;

#[async_trait::async_trait]
pub(crate) trait EventHandler<T> {
    async fn handle(&mut self, event: T) -> EventResult;
}

#[async_trait::async_trait]
impl EventHandler<Event> for Runtime {
    async fn handle(&mut self, event: Event) -> EventResult {
        if let Err(error) = self.event_sender.try_send(event) {
            warn!(reason = %error, "Unable to send NetworkEvent event to outer stream");
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl EventHandler<ComposedEvent> for Runtime {
    async fn handle(&mut self, event: ComposedEvent) -> EventResult {
        match event {
            ComposedEvent::Kademlia(event) => self.handle(event).await,
            ComposedEvent::PeerInfo(event) => self.handle(event).await,
            ComposedEvent::Gossipsub(event) => self.handle(event).await,
            ComposedEvent::Grpc(event) => self.handle(event).await,
            ComposedEvent::Void => Ok(()),
        }
    }
}

#[async_trait::async_trait]
impl EventHandler<SwarmEvent<ComposedEvent>> for Runtime {
    async fn handle(&mut self, event: SwarmEvent<ComposedEvent>) -> EventResult {
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

            SwarmEvent::OutgoingConnectionError {
                connection_id,
                peer_id: Some(peer_id),
                error,
            } if self
                .health_state
                .successfully_connected_to_bootpeer
                .is_none()
                && self.health_state.dialed_bootpeer.contains(&connection_id) =>
            {
                warn!("Unable to connect to bootpeer {peer_id}: {error:?}");
                self.health_state.dialed_bootpeer.remove(&connection_id);
                if self.health_state.dialed_bootpeer.is_empty() {
                    // We tried to connect to all bootnode without success
                    error!("Unable to connect to any bootnode");
                }
            }

            SwarmEvent::OutgoingConnectionError {
                peer_id,
                error,
                connection_id,
            } => {
                if let Some(peer_id) = peer_id {
                    error!(
                        "OutgoingConnectionError peer_id: {peer_id} | error: {error:?} | \
                         connection_id: {connection_id}"
                    );
                } else {
                    error!(
                        "OutgoingConnectionError for unknown peer | error: {error:?} | \
                         connection_id: {connection_id}"
                    );
                    error!("OutgoingConnectionError {error:?}");
                }
            }

            SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                endpoint,
                num_established,
                concurrent_dial_errors,
                established_in,
            } if self.health_state.dialed_bootpeer.contains(&connection_id) => {
                info!("Successfully connected to bootpeer {peer_id}");
                if self
                    .health_state
                    .successfully_connected_to_bootpeer
                    .is_none()
                {
                    self.health_state.successfully_connected_to_bootpeer = Some(connection_id);
                    _ = self.health_state.dialed_bootpeer.remove(&connection_id);
                }
            }

            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!(
                    "Connection established with peer {peer_id} as {:?}",
                    endpoint.to_endpoint()
                );

                if self.swarm.connected_peers().count() >= self.config.minimum_cluster_size {
                    if let Err(error) = self.swarm.behaviour_mut().gossipsub.subscribe() {
                        error!("Unable to subscribe to gossipsub topic: {}", error);

                        return Err(P2PError::GossipTopicSubscriptionFailure);
                    }
                }
            }

            incoming_connection_error @ SwarmEvent::IncomingConnectionError { .. } => {
                error!("{:?}", incoming_connection_error);
            }

            SwarmEvent::IncomingConnection {
                local_addr,
                connection_id,
                send_back_addr,
            } => {
                debug!(
                    "IncomingConnection | local_addr: {local_addr} | connection_id: \
                     {connection_id} | send_back_addr: {send_back_addr}"
                )
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

            SwarmEvent::Dialing {
                peer_id: Some(ref peer_id),
                connection_id,
            } if self.boot_peers.contains(peer_id) => {
                info!("Dialing bootpeer {peer_id} on connection: {connection_id}");
                self.health_state.dialed_bootpeer.insert(connection_id);
            }

            SwarmEvent::Dialing {
                peer_id,
                connection_id,
            } => {
                debug!("Dialing peer_id: {peer_id:?} | connection_id: {connection_id}");
            }

            SwarmEvent::Behaviour(event) => self.handle(event).await?,

            SwarmEvent::ExpiredListenAddr {
                listener_id,
                address,
            } => error!("Unhandled ExpiredListenAddr {listener_id:?} | {address}"),

            SwarmEvent::ListenerError { listener_id, error } => {
                error!("Unhandled ListenerError {listener_id:?} | {error}")
            }
            event => {
                warn!("Unhandled SwarmEvent: {:?}", event);
            }
        }

        let behaviour = self.swarm.behaviour();

        if let Some(event) = self.healthy_status_changed() {
            debug!("Healthy status changed: {:?}", event);
            _ = self.event_sender.send(event).await;
        }

        info!("Healthystatus: {:?}", self.health_status);
        Ok(())
    }
}
