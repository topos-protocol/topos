use std::{future::IntoFuture, time::Duration};

use builder::GatekeeperBuilder;
use futures::{future::BoxFuture, FutureExt};
use rand::{seq::SliceRandom, thread_rng};
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot},
    time,
};

mod builder;
mod client;
#[cfg(test)]
mod tests;

pub use client::GatekeeperClient;
use topos_commands::{Command, CommandHandler, RegisterCommands};
use topos_p2p::PeerId;

pub struct Gatekeeper {
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,
    pub(crate) commands: mpsc::Receiver<GatekeeperCommand>,
    pub(crate) tick_duration: Duration,

    peer_list: Vec<PeerId>,
}

impl Default for Gatekeeper {
    fn default() -> Self {
        let (_shutdown_channel, shutdown) = mpsc::channel(1);
        let (_commands, commands_recv) = mpsc::channel(1);
        let tick_duration = Duration::from_secs(Self::DEFAULT_TICK_DURATION);

        Self {
            shutdown,
            commands: commands_recv,
            tick_duration,
            peer_list: Vec::default(),
        }
    }
}

#[async_trait::async_trait]
impl CommandHandler<PushPeerList> for Gatekeeper {
    type Error = GatekeeperError;

    async fn handle(
        &mut self,
        PushPeerList { mut peer_list }: PushPeerList,
    ) -> Result<(), Self::Error> {
        peer_list.dedup();

        self.peer_list = peer_list.into_iter().collect();

        Ok(())
    }
}

#[async_trait::async_trait]
impl CommandHandler<GetAllPeers> for Gatekeeper {
    type Error = GatekeeperError;

    async fn handle(&mut self, _command: GetAllPeers) -> Result<Vec<PeerId>, Self::Error> {
        Ok(self.peer_list.clone())
    }
}

#[async_trait::async_trait]
impl CommandHandler<GetRandomPeers> for Gatekeeper {
    type Error = GatekeeperError;

    async fn handle(
        &mut self,
        GetRandomPeers { number }: GetRandomPeers,
    ) -> Result<Vec<PeerId>, Self::Error> {
        let peer_list_len = self.peer_list.len();

        if number > peer_list_len {
            return Err(GatekeeperError::InvalidCommand(format!(
                "Asked for {number} random peers when the Gatekeeper have {peer_list_len}"
            )));
        }

        let mut range: Vec<u32> = (0..(peer_list_len as u32)).collect();
        range.shuffle(&mut thread_rng());

        let iterator = range.iter().take(number);

        let mut peers = Vec::new();

        for index in iterator {
            if let Some(peer) = self.peer_list.get(*index as usize) {
                peers.push(*peer);
            }
        }

        Ok(peers)
    }
}

impl IntoFuture for Gatekeeper {
    type Output = Result<(), GatekeeperError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            let mut interval = time::interval(self.tick_duration);

            let shutdowned: Option<oneshot::Sender<()>> = loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    sender = self.shutdown.recv() => {
                        break sender;
                    }
                    Some(command) = self.commands.recv() => match command {
                         GatekeeperCommand::PushPeerList(command, response_channel) => {
                            _ = response_channel.send(self.handle(command).await)
                        },
                        GatekeeperCommand::GetAllPeers(command, response_channel) => {
                            _ = response_channel.send(self.handle(command).await)
                        },
                        GatekeeperCommand::GetRandomPeers(command, response_channel) => {
                            _ = response_channel.send(self.handle(command).await)
                        },

                    }
                }
            };

            if let Some(sender) = shutdowned {
                _ = sender.send(());
            }

            Ok(())
        }
        .boxed()
    }
}

impl Gatekeeper {
    pub(crate) const DEFAULT_TICK_DURATION: u64 = 10;

    #[allow(dead_code)]
    pub fn builder() -> GatekeeperBuilder {
        GatekeeperBuilder {}
    }
}

#[derive(Debug, Error)]
pub enum GatekeeperError {
    #[error("Unable to communicate with Gatekeeper: {0}")]
    CommunicationChannel(#[from] mpsc::error::SendError<GatekeeperCommand>),

    #[error("Unable to receive expected response from Gatekeeper: {0}")]
    ResponseChannel(#[from] oneshot::error::RecvError),

    #[error("Unable to execute command on the Gatekeeper: {0}")]
    InvalidCommand(String),
}

RegisterCommands!(
    name = GatekeeperCommand,
    error = GatekeeperError,
    commands = [PushPeerList, GetAllPeers, GetRandomPeers]
);

#[derive(Debug)]
pub struct PushPeerList {
    peer_list: Vec<PeerId>,
}

impl Command for PushPeerList {
    type Result = ();
}

#[derive(Debug)]
pub struct GetAllPeers;

impl Command for GetAllPeers {
    type Result = Vec<PeerId>;
}

#[derive(Debug)]
pub struct GetRandomPeers {
    number: usize,
}

impl Command for GetRandomPeers {
    type Result = Vec<PeerId>;
}
