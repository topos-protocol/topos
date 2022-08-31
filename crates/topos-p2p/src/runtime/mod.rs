use crate::{Behaviour, Command, Event};
use libp2p::Swarm;
use std::error::Error;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use topos_addr::ToposAddr;

pub struct Runtime {
    swarm: Swarm<Behaviour>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<Event>,
}

mod handle_command;
mod handle_event;

impl Runtime {
    pub(crate) fn new(
        swarm: Swarm<Behaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<Event>,
    ) -> Self {
        Self {
            swarm,
            command_receiver,
            event_sender,
        }
    }

    fn start_listening(&mut self, peer_addr: ToposAddr) -> Result<(), Box<dyn Error + Send>> {
        match self.swarm.listen_on(peer_addr.inner()) {
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
