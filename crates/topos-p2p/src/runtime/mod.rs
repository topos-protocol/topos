use std::collections::{HashMap, HashSet};

use crate::{
    behaviour::{
        discovery::{PendingDials, PendingRecordRequest},
        transmission::PendingRequests,
    },
    error::P2PError,
    runtime::handle_event::EventHandler,
    Behaviour, Command, Event,
};
use libp2p::{core::transport::ListenerId, kad::QueryId, Multiaddr, PeerId, Swarm};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::error;

pub struct Runtime {
    pub(crate) swarm: Swarm<Behaviour>,
    pub(crate) command_receiver: mpsc::Receiver<Command>,
    pub(crate) event_sender: mpsc::Sender<Event>,
    pub(crate) local_peer_id: PeerId,
    pub(crate) listening_on: Multiaddr,
    pub(crate) addresses: Multiaddr,
    pub(crate) bootstrapped: bool,

    pub(crate) pending_requests: PendingRequests,

    /// Contains peer ids of dialled node
    pub peers: HashSet<PeerId>,

    /// Holds the pending dials and their sender
    pub pending_dial: PendingDials,

    /// Contains current listenerId of the swarm
    pub active_listeners: HashSet<ListenerId>,

    /// Pending DHT queries
    pub pending_record_requests: HashMap<QueryId, PendingRecordRequest>,
}

mod handle_command;
mod handle_event;

impl Runtime {
    fn start_listening(&mut self, peer_addr: Multiaddr) -> Result<(), P2PError> {
        self.swarm
            .listen_on(peer_addr)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn run(mut self) {
        let addr = self.listening_on.clone();
        if let Err(error) = self.swarm.listen_on(addr) {
            error!(
                "Couldn't start listening on {} because of {error:?}",
                self.listening_on
            );
        }

        loop {
            tokio::select! {
                Some(event) = self.swarm.next() => self.handle(event).await,
                command = self.command_receiver.recv() =>
                    match command {
                        Some(command) => self.handle_command(command).await,
                        None => return,
                    }
            }
        }
    }
}
