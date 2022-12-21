use std::future::IntoFuture;

use builder::CheckpointsCollectorBuilder;
use futures::{future::BoxFuture, FutureExt};
use thiserror::Error;
use tokio::sync::mpsc;

mod builder;
mod client;

pub use client::CheckpointsCollectorClient;

pub struct CheckpointsCollector {
    pub(crate) shutdown: mpsc::Receiver<()>,
    pub(crate) commands: mpsc::Receiver<CheckpointsCollectorCommand>,
    #[allow(dead_code)]
    pub(crate) events: mpsc::Sender<CheckpointsCollectorEvent>,
}

impl IntoFuture for CheckpointsCollector {
    type Output = Result<(), CheckpointsCollectorError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            loop {
                tokio::select! {
                    _ = self.shutdown.recv() => { break; }
                    _command = self.commands.recv() => {}
                }
            }

            Ok(())
        }
        .boxed()
    }
}

impl CheckpointsCollector {
    #[allow(dead_code)]
    pub fn builder() -> CheckpointsCollectorBuilder {
        CheckpointsCollectorBuilder {}
    }
}

#[derive(Error, Debug)]
pub enum CheckpointsCollectorError {
    #[error("Unable to start the CheckpointsCollector")]
    UnableToStart,
}
pub enum CheckpointsCollectorCommand {}
pub enum CheckpointsCollectorEvent {}
