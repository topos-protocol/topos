use std::collections::{HashMap, HashSet};

use crate::{
    behaviour::{discovery::PendingRecordRequest, HealthStatus},
    config::NetworkConfig,
    error::P2PError,
    runtime::handle_event::EventHandler,
    Behaviour, Command, Event,
};
use libp2p::{
    core::transport::ListenerId, kad::QueryId, swarm::ConnectionId, Multiaddr, PeerId, Swarm,
};
use tokio::{
    spawn,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tokio_stream::{Stream, StreamExt};
use tracing::{debug, error, info, Instrument};

pub struct Runtime {
    pub(crate) config: NetworkConfig,
    // TODO: check if needed
    pub(crate) peer_set: HashSet<PeerId>,
    pub(crate) swarm: Swarm<Behaviour>,
    pub(crate) command_receiver: mpsc::Receiver<Command>,
    pub(crate) event_sender: mpsc::Sender<Event>,
    pub(crate) local_peer_id: PeerId,
    pub(crate) listening_on: Vec<Multiaddr>,
    pub(crate) public_addresses: Vec<Multiaddr>,

    /// Well-known or pre-configured bootnodes to connect to in order to bootstrap the p2p layer
    pub(crate) boot_peers: Vec<PeerId>,

    /// Contains current listenerId of the swarm
    pub active_listeners: HashSet<ListenerId>,

    /// Pending DHT queries
    pub pending_record_requests: HashMap<QueryId, PendingRecordRequest>,

    /// Shutdown signal receiver from the client
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,

    /// Internal health state of the p2p layer
    pub(crate) health_state: HealthState,

    /// Health status of the p2p layer
    pub(crate) health_status: HealthStatus,
}

mod handle_command;
mod handle_event;

/// Internal health state of the p2p layer
///
/// This struct may change in the future to be more flexible and to handle more
/// complex state transitions/representation.
#[derive(Default)]
pub(crate) struct HealthState {
    /// Indicates if the node has external addresses configured
    pub(crate) has_external_addresses: bool,
    /// Indicates if the node is listening on any address
    pub(crate) is_listening: bool,
    /// List the bootnodes that the node has tried to connect to
    pub(crate) dialed_bootpeer: HashSet<ConnectionId>,
    /// Indicates if the node has successfully connected to a bootnode
    pub(crate) successfully_connected_to_bootpeer: Option<ConnectionId>,
    /// Track the number of remaining retries to connect to any bootnode
    pub(crate) bootpeer_connection_retries: usize,
}

impl Runtime {
    /// Bootstrap the p2p layer runtime with the given configuration.
    /// This method will configure, launch and start queries.
    /// The result of this call is a p2p layer bootstrap but it doesn't mean it is
    /// ready.
    pub async fn bootstrap<S: Stream<Item = Event> + Unpin + Send>(
        mut self,
        event_stream: &mut S,
    ) -> Result<JoinHandle<Result<(), P2PError>>, P2PError> {
        debug!("Added public addresses: {:?}", self.public_addresses);
        for address in &self.public_addresses {
            self.swarm.add_external_address(address.clone());
            self.health_state.has_external_addresses = true;
        }
        debug!("Starting to listen on {:?}", self.listening_on);

        for addr in &self.listening_on {
            if let Err(error) = self.swarm.listen_on(addr.clone()) {
                error!("Couldn't start listening on {} because of {error:?}", addr);

                return Err(P2PError::TransportError(error));
            }

            self.health_state.is_listening = true;
        }

        let mut handle = spawn(self.run().in_current_span());

        // Await the Event::Healthy coming from freshly started p2p layer
        loop {
            tokio::select! {
                result = &mut handle => {
                    match result {
                        Ok(Ok(_)) => info!("P2P layer has been shutdown"),
                        Ok(Err(error)) => {
                            error!("P2P layer has failed with error: {:?}", error);

                            return Err(error);
                        }
                        Err(_) => {
                            error!("P2P layer has failed in an unexpected way.");
                            return Err(P2PError::JoinHandleFailure);
                        }
                    }
                }
                Some(event) = event_stream.next() => {
                    if let Event::Healthy = event {
                        info!("P2P layer is healthy");
                        break;
                    }
                }
            }
        }

        Ok(handle)
    }

    /// Run p2p runtime
    pub async fn run(mut self) -> Result<(), P2PError> {
        let shutdowned: Option<oneshot::Sender<()>> = loop {
            tokio::select! {
                Some(event) = self.swarm.next() => {
                    self.handle(event).in_current_span().await?
                },
                Some(command) = self.command_receiver.recv() => self.handle_command(command).in_current_span().await,
                shutdown = self.shutdown.recv() => {
                    break shutdown;
                }
            }
        };

        if let Some(sender) = shutdowned {
            info!("Shutting down p2p runtime...");
            _ = sender.send(());
        }

        Ok(())
    }

    pub(crate) fn healthy_status_changed(&mut self) -> Option<Event> {
        let behaviours = self.swarm.behaviour();
        let gossipsub = &behaviours.gossipsub.health_status;
        let discovery = &behaviours.discovery.health_status;

        let new_status = match (discovery, gossipsub) {
            (HealthStatus::Killing, _) | (_, HealthStatus::Killing) => HealthStatus::Killing,
            (HealthStatus::Initializing, _) | (_, HealthStatus::Initializing) => {
                HealthStatus::Initializing
            }
            (HealthStatus::Unhealthy, _) | (_, HealthStatus::Unhealthy) => HealthStatus::Unhealthy,
            (HealthStatus::Recovering, _) | (_, HealthStatus::Recovering) => {
                HealthStatus::Recovering
            }
            (HealthStatus::Healthy, HealthStatus::Healthy) => HealthStatus::Healthy,
        };

        if self.health_status != new_status {
            self.health_status = new_status;

            Some((&self.health_status).into())
        } else {
            None
        }
    }
}
