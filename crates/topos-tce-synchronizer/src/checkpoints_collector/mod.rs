use std::future::IntoFuture;

use builder::CheckpointsCollectorBuilder;
use futures::{future::BoxFuture, FutureExt};
use thiserror::Error;
use tokio::sync::{
    mpsc::{self, error::SendError},
    oneshot::{self, error::RecvError},
};

mod builder;
mod client;

pub use client::CheckpointsCollectorClient;
use topos_p2p::Client as NetworkClient;
use topos_tce_gatekeeper::GatekeeperClient;

pub struct CheckpointsCollectorConfig {
    peer_number: usize,
}

impl CheckpointsCollectorConfig {
    const DEFAULT_PEER_NUMBER: usize = 10;
}

impl Default for CheckpointsCollectorConfig {
    fn default() -> Self {
        Self {
            peer_number: Self::DEFAULT_PEER_NUMBER,
        }
    }
}

pub struct CheckpointsCollector {
    started: bool,
    config: CheckpointsCollectorConfig,
    #[allow(dead_code)]
    network: NetworkClient,

    pub(crate) shutdown: mpsc::Receiver<()>,
    pub(crate) commands: mpsc::Receiver<CheckpointsCollectorCommand>,
    #[allow(dead_code)]
    pub(crate) events: mpsc::Sender<CheckpointsCollectorEvent>,
    pub(crate) gatekeeper: GatekeeperClient,
}

impl IntoFuture for CheckpointsCollector {
    type Output = Result<(), CheckpointsCollectorError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            loop {
                tokio::select! {
                    _ = self.shutdown.recv() => { break; }
                    Some(command) = self.commands.recv() => self.handle(command).await?
                }
            }

            Ok(())
        }
        .boxed()
    }
}

impl CheckpointsCollector {
    pub fn builder() -> CheckpointsCollectorBuilder {
        CheckpointsCollectorBuilder::default()
    }

    async fn handle(
        &mut self,
        command: CheckpointsCollectorCommand,
    ) -> Result<(), CheckpointsCollectorError> {
        match command {
            CheckpointsCollectorCommand::StartCollecting { response_channel } if !self.started => {
                self.started = true;

                _ = response_channel.send(Ok(()));

                // Need to fetch random peers
                let _peers = match self
                    .gatekeeper
                    .get_random_peers(self.config.peer_number)
                    .await
                {
                    Ok(peers) => peers,
                    Err(_) => return Err(CheckpointsCollectorError::UnableToFetchRandomPeer),
                };
            }
            CheckpointsCollectorCommand::StartCollecting { response_channel } => {
                _ = response_channel.send(Err(CheckpointsCollectorError::AlreadyStarting));
            }
        }

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum CheckpointsCollectorError {
    #[error("Unable to start the CheckpointsCollector")]
    UnableToStart,

    #[error("Unable to start the CheckpointsCollector: No gatekeeper client provided")]
    NoGatekeeperClient,

    #[error("Unable to start the CheckpointsCollector: No network client provided")]
    NoNetworkClient,

    #[error("Error while dealing with Start command: already starting")]
    AlreadyStarting,

    #[error("Error while trying to fetch random peers")]
    UnableToFetchRandomPeer,

    #[error(transparent)]
    OneshotCommunicationChannel(#[from] RecvError),

    #[error(transparent)]
    InternalCommunicationChannel(#[from] SendError<CheckpointsCollectorCommand>),
}

#[derive(Debug)]
pub enum CheckpointsCollectorCommand {
    StartCollecting {
        response_channel: oneshot::Sender<Result<(), CheckpointsCollectorError>>,
    },
}

pub enum CheckpointsCollectorEvent {}
