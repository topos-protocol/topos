use crate::{Behaviour, Command, Event};
use libp2p::{Multiaddr, PeerId, Swarm};
use std::error::Error;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

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

    fn start_listening(&mut self, peer_addr: Multiaddr) -> Result<(), Box<dyn Error + Send>> {
        match self.swarm.listen_on(peer_addr) {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                event = self.swarm.next() => self.handle_event(event.unwrap()).await,
                command = self.command_receiver.recv() =>
                    match command {
                        Some(command) => self.handle_command(command).await,
                        None => return,
                    }
            }
        }
    }
}
