use std::{future::IntoFuture, time::Duration};

use builder::GatekeeperBuilder;
use futures::{future::BoxFuture, FutureExt};
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
}

#[async_trait::async_trait]
impl CommandHandler<PushPeerList> for Gatekeeper {
    type Error = GatekeeperError;

    async fn handle(&mut self, _command: PushPeerList) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl CommandHandler<GetAllPeers> for Gatekeeper {
    type Error = GatekeeperError;

    async fn handle(&mut self, _command: GetAllPeers) -> Result<Vec<PeerId>, Self::Error> {
        Ok(vec![])
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
}

RegisterCommands!(
    GatekeeperCommand,
    GatekeeperError,
    PushPeerList,
    GetAllPeers
);

#[derive(Debug)]
pub struct PushPeerList(Vec<PeerId>);

impl Command for PushPeerList {
    type Result = ();
}

#[derive(Debug)]
pub struct GetAllPeers;

impl Command for GetAllPeers {
    type Result = Vec<PeerId>;
}
