use crate::{error::P2PError, Behaviour, Command, Event};
use libp2p::{Multiaddr, PeerId, Swarm};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::error;

pub struct Runtime {
    swarm: Swarm<Behaviour>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
    local_peer_id: PeerId,
}

mod handle_command;
mod handle_event;

impl Runtime {
    pub(crate) fn new(
        swarm: Swarm<Behaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
        local_peer_id: PeerId,
    ) -> Self {
        Self {
            swarm,
            command_receiver,
            event_sender,
            local_peer_id,
        }
    }

    fn start_listening(&mut self, peer_addr: Multiaddr) -> Result<(), P2PError> {
        self.swarm
            .listen_on(peer_addr)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn run(mut self) {
        let addr = self.swarm.behaviour().addresses.clone();
        if let Err(error) = self.swarm.listen_on(addr) {
            error!(
                "Couldn't start listening on {} because of {error:?}",
                self.swarm.behaviour().addresses
            );
        }

        loop {
            tokio::select! {
                Some(event) = self.swarm.next() => self.handle_event(event).await,
                command = self.command_receiver.recv() =>
                    match command {
                        Some(command) => self.handle_command(command).await,
                        None => return,
                    }
            }
        }
    }
}
