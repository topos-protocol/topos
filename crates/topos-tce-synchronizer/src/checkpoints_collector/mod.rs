use std::future::IntoFuture;

use builder::CheckpointsCollectorBuilder;
use futures::{future::BoxFuture, stream::FuturesUnordered, FutureExt};
use thiserror::Error;
use tokio::sync::{
    mpsc::{self, error::SendError},
    oneshot::{self, error::RecvError},
};
use tracing::error;

mod builder;
mod client;

pub use client::CheckpointsCollectorClient;
use tokio_stream::StreamExt;
use topos_api::{
    shared::v1::Uuid as APIUuid,
    tce::v1::{
        checkpoint_request::{self, RequestType},
        CheckpointRequest, CheckpointResponse,
    },
};
use topos_p2p::{error::CommandExecutionError, Client as NetworkClient, RetryPolicy};
use topos_tce_gatekeeper::GatekeeperClient;
use uuid::Uuid;

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

    current_request_id: Option<APIUuid>,

    pending_checkpoint_requests:
        FuturesUnordered<BoxFuture<'static, Result<CheckpointResponse, CommandExecutionError>>>,

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
                    Some(response) = self.pending_checkpoint_requests.next() => {
                        self.handle_checkpoint_response(response).await;
                    }
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

    async fn handle_checkpoint_response(
        &mut self,
        response: Result<CheckpointResponse, CommandExecutionError>,
    ) {
        match response {
            Ok(response) => {
                if response.request_id != self.current_request_id {
                    return;
                }
                // Handle the valid request here
                let _heads = response.positions;
            }
            Err(_error) => {
                // TODO: add context about the error
                error!("Received a command execution error when dealing with checkpoint");
            }
        }
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
                let peers = match self
                    .gatekeeper
                    .get_random_peers(self.config.peer_number)
                    .await
                {
                    Ok(peers) => peers,
                    Err(_) => return Err(CheckpointsCollectorError::UnableToFetchRandomPeer),
                };

                self.current_request_id = Some(Uuid::new_v4().into());

                // upon receiving random peers send network messages
                for peer in peers {
                    // Sending CheckpointRequest
                    let req = CheckpointRequest {
                        request_id: self.current_request_id,
                        request_type: Some(RequestType::Heads(checkpoint_request::Heads {
                            subnet_ids: vec![],
                        })),
                    };

                    // TODO: Put rate limit on the futures pool
                    self.pending_checkpoint_requests.push(
                        self.network
                            .send_request::<_, CheckpointResponse>(peer, req, RetryPolicy::NoRetry)
                            .boxed(),
                    );
                }
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
