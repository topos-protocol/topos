use std::future::IntoFuture;

use builder::SynchronizerBuilder;
use checkpoints_collector::{CheckpointsCollectorError, CheckpointsCollectorEvent};
use futures::{future::BoxFuture, FutureExt};
use thiserror::Error;
use tokio::sync::{
    mpsc,
    oneshot::{self, error::RecvError},
};
use tokio_stream::StreamExt;

mod builder;
mod checkpoints_collector;

use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub struct Synchronizer {
    pub(crate) shutdown: CancellationToken,
    #[allow(dead_code)]
    pub(crate) events: mpsc::Sender<SynchronizerEvent>,

    pub(crate) checkpoints_collector_stream: ReceiverStream<CheckpointsCollectorEvent>,
}

impl IntoFuture for Synchronizer {
    type Output = Result<(), SynchronizerError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            let shutdowned: Option<SynchronizerError> = loop {
                tokio::select! {
                    _ = self.shutdown.cancelled() => {
                        break None
                    }
                    _checkpoint_event = self.checkpoints_collector_stream.next() => {}
                }
            };

            if let Some(_error) = shutdowned {
                warn!("Shutting down Synchronizer due to error...");
            } else {
                info!("Shutting down Synchronizer...");
            }

            Ok(())
        }
        .boxed()
    }
}

impl Synchronizer {
    pub fn builder() -> SynchronizerBuilder {
        SynchronizerBuilder::default()
    }
}

#[derive(Error, Debug)]
pub enum SynchronizerError {
    #[error("Error while dealing with CheckpointsCollector: {0}")]
    CheckpointsCollectorError(#[from] CheckpointsCollectorError),

    #[error("Error while dealing with Start command: unable to start")]
    UnableToStart,

    #[error("Error while dealing with Start command: already starting")]
    AlreadyStarting,

    #[error("Error while dealing with state locking: unable to lock status")]
    UnableToLockStatus,

    #[error(transparent)]
    OneshotCommunicationChannel(#[from] RecvError),

    #[error("Unable to execute shutdown on the Synchronizer: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),
}

pub enum SynchronizerEvent {}
