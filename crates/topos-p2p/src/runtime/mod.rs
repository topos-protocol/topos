use std::collections::{HashMap, HashSet};

use crate::{
    behaviour::discovery::PendingRecordRequest, config::NetworkConfig,
    runtime::handle_event::EventHandler, Behaviour, Command, Event,
};
use libp2p::{core::transport::ListenerId, kad::QueryId, Multiaddr, PeerId, Swarm};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use tracing::{debug, error, info};

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

    /// Contains current listenerId of the swarm
    pub active_listeners: HashSet<ListenerId>,

    /// Pending DHT queries
    pub pending_record_requests: HashMap<QueryId, PendingRecordRequest>,

    /// Shutdown signal receiver from the client
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,
}

mod handle_command;
mod handle_event;

impl Runtime {
    /// Bootstrap the p2p layer runtime with the given configuration.
    /// This method will configure, launch and start queries.
    /// The result of this call is a p2p layer bootstrap but it doesn't mean it is
    /// ready.
    pub async fn bootstrap(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Added public addresses: {:?}", self.public_addresses);
        for address in &self.public_addresses {
            self.swarm.add_external_address(address.clone());
        }

        debug!("Starting to listen on {:?}", self.listening_on);
        for addr in &self.listening_on {
            if let Err(error) = self.swarm.listen_on(addr.clone()) {
                error!("Couldn't start listening on {} because of {error:?}", addr);

                return Err(Box::new(error));
            }
        }

        if !self.peer_set.is_empty() {
            debug!(
                "{} Connected to some peers, start a bootstrap query",
                self.local_peer_id
            );
            self.swarm.behaviour_mut().discovery.bootstrap()?;
        }

        while let Some(event) = self.swarm.next().await {
            self.handle(event).await;

            if self.swarm.connected_peers().count() >= self.config.minimum_cluster_size {
                break;
            }
        }

        let gossipsub = &mut self.swarm.behaviour_mut().gossipsub;

        gossipsub.subscribe()?;

        Ok(())
    }

    /// Run p2p runtime
    pub async fn run(mut self) -> Result<(), ()> {
        let shutdowned: Option<oneshot::Sender<()>> = loop {
            tokio::select! {
                Some(event) = self.swarm.next() => self.handle(event).await,
                Some(command) = self.command_receiver.recv() => self.handle_command(command).await,
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
}
