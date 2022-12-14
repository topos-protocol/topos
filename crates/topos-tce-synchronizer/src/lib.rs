use std::future::IntoFuture;

use builder::SynchronizerBuilder;
use checkpoints_collector::{
    CheckpointsCollectorClient, CheckpointsCollectorError, CheckpointsCollectorEvent,
};
use futures::{future::BoxFuture, FutureExt};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

mod builder;
mod checkpoints_collector;
mod client;

pub use client::SynchronizerClient;
use tokio_stream::wrappers::ReceiverStream;

pub struct Synchronizer {
    pub(crate) shutdown: mpsc::Receiver<()>,
    pub(crate) commands: mpsc::Receiver<SynchronizerCommand>,
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
            loop {
                tokio::select! {
                    _ = self.shutdown.recv() => { break; }
                    _command = self.commands.recv() => {}
                    _checkpoint_event = self.checkpoints_collector_stream.next() => {}
                }
            }

            Ok(())
        }
        .boxed()
    }
}

impl Synchronizer {
    #[allow(dead_code)]
    pub fn builder() -> SynchronizerBuilder {
        SynchronizerBuilder {}
    }
}

#[derive(Error, Debug)]
pub enum SynchronizerError {
    #[error("Error while dealing with CheckpointsCollector: {0}")]
    CheckpointsCollectorError(#[from] CheckpointsCollectorError),
}
pub enum SynchronizerCommand {}
pub enum SynchronizerEvent {}
