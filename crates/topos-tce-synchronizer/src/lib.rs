use std::future::IntoFuture;

use builder::SynchronizerBuilder;
use checkpoints_collector::{
    CheckpointsCollectorClient, CheckpointsCollectorError, CheckpointsCollectorEvent,
};
use futures::{future::BoxFuture, FutureExt};
use thiserror::Error;
use tokio::sync::{
    mpsc,
    oneshot::{self, error::RecvError},
};
use tokio_stream::StreamExt;

mod builder;
mod checkpoints_collector;
mod client;

pub use client::SynchronizerClient;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, warn};

pub struct Synchronizer {
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,
    #[allow(dead_code)]
    pub(crate) events: mpsc::Sender<SynchronizerEvent>,

    #[allow(dead_code)]
    pub(crate) checkpoints_collector: CheckpointsCollectorClient,
    pub(crate) checkpoints_collector_stream: ReceiverStream<CheckpointsCollectorEvent>,
}

impl IntoFuture for Synchronizer {
    type Output = Result<(), SynchronizerError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            let shutdowned: Option<oneshot::Sender<()>> = loop {
                tokio::select! {

                    shutdown = self.shutdown.recv() => {
                        break shutdown;
                    }
                    _checkpoint_event = self.checkpoints_collector_stream.next() => {}
                }
            };

            if let Some(sender) = shutdowned {
                info!("Shutting down Synchronizer...");
                _ = sender.send(());
            } else {
                warn!("Shutting down Synchronizer due to error...");
            }

            Ok(())
        }
        .boxed()
    }
}

impl Synchronizer {
    #[allow(dead_code)]
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
