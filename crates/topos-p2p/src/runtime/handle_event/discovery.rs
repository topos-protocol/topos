use libp2p::kad::{BootstrapOk, BootstrapResult, Event, QueryResult};
use tracing::{debug, error, info, warn};

use crate::{behaviour::HealthStatus, error::P2PError, Runtime};

use super::{EventHandler, EventResult};

#[async_trait::async_trait]
impl EventHandler<Box<Event>> for Runtime {
    async fn handle(&mut self, event: Box<Event>) -> EventResult {
        match *event {
            Event::InboundRequest { request } => {
                // warn!("InboundRequest {:?}", request);
            }

            Event::RoutingUpdated {
                peer, addresses, ..
            } => {
                debug!("DHT -> RoutingUpdated {:?} {:?}", peer, addresses);
            }

            Event::RoutablePeer { peer, address } => {
                debug!("DHT -> RoutablePeer {:?}, {:?}", peer, address);
            }

            Event::PendingRoutablePeer { peer, address } => {
                debug!("DHT -> PendingRoutablePeer {:?}, {:?}", peer, address);
            }

            Event::UnroutablePeer { peer } => {
                // Ignored
            }

            Event::OutboundQueryProgressed {
                id,
                result:
                    QueryResult::Bootstrap(BootstrapResult::Ok(BootstrapOk {
                        peer,
                        num_remaining,
                    })),
                stats,
                step,
            } if num_remaining == 0
                && self.swarm.behaviour().discovery.health_status == HealthStatus::Initializing =>
            {
                if self
                    .health_state
                    .successfully_connected_to_bootpeer
                    .is_none()
                {
                    warn!(
                        "Bootstrap query finished but unable to connect to bootnode during \
                         initialization, switching from discovery(initializing) -> \
                         discover(unhealthy) and fast bootstrap mode",
                    );

                    let behaviour = self.swarm.behaviour_mut();

                    behaviour.discovery.health_status = HealthStatus::Unhealthy;
                    _ = behaviour
                        .discovery
                        .change_interval(self.config.discovery.fast_bootstrap_interval)
                        .await;
                } else {
                    warn!(
                        "Bootstrap query finished with bootnode, switching from \
                         discovery(initializing) -> discovery(healthy)",
                    );

                    let behaviour = self.swarm.behaviour_mut();

                    behaviour.discovery.health_status = HealthStatus::Healthy;
                }
            }

            Event::OutboundQueryProgressed {
                id,
                result:
                    QueryResult::Bootstrap(BootstrapResult::Ok(BootstrapOk {
                        peer,
                        num_remaining,
                    })),
                stats,
                step,
            } if num_remaining == 0
                && self
                    .health_state
                    .successfully_connected_to_bootpeer
                    .is_none()
                && self.swarm.behaviour().discovery.health_status == HealthStatus::Unhealthy =>
            {
                match self.health_state.bootpeer_connection_retries.checked_sub(1) {
                    None => {
                        error!(
                            "Bootstrap query finished but unable to connect to bootnode, stopping"
                        );

                        return Err(P2PError::UnableToReachBootnode);
                    }
                    Some(new) => {
                        warn!(
                            "Bootstrap query finished but unable to connect to bootnode, retrying \
                             {} more times",
                            new
                        );
                        self.health_state.bootpeer_connection_retries = new;
                    }
                }
            }
            Event::OutboundQueryProgressed {
                id,
                result:
                    QueryResult::Bootstrap(BootstrapResult::Ok(BootstrapOk {
                        peer,
                        num_remaining,
                    })),
                stats,
                step,
            } if num_remaining == 0
                && self
                    .health_state
                    .successfully_connected_to_bootpeer
                    .is_some()
                && self.swarm.behaviour().discovery.health_status == HealthStatus::Unhealthy =>
            {
                info!(
                    "Bootstrap query finished with bootnode, switching discover(unhealthy) -> \
                     discover(healthy) and normal bootstrap mode",
                );

                let behaviour = self.swarm.behaviour_mut();
                behaviour.discovery.health_status = HealthStatus::Healthy;
                _ = behaviour
                    .discovery
                    .change_interval(self.config.discovery.bootstrap_interval)
                    .await;
            }

            Event::OutboundQueryProgressed {
                result: QueryResult::Bootstrap(res),
                id,
                ..
            } => {
                debug!("BootstrapResult query: {id:?},  {res:?}");
            }

            Event::OutboundQueryProgressed {
                id, result, stats, ..
            } => {}
            Event::ModeChanged { new_mode } => {}
        }

        Ok(())
    }
}
