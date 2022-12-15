use std::future::IntoFuture;

use builder::SynchronizerBuilder;
use futures::{future::BoxFuture, FutureExt};
use thiserror::Error;
use tokio::sync::mpsc;

mod builder;
mod client;

pub use client::SynchronizerClient;

pub struct Synchronizer {
    pub(crate) shutdown: mpsc::Receiver<()>,
    pub(crate) commands: mpsc::Receiver<SynchronizerCommand>,
    #[allow(dead_code)]
    pub(crate) events: mpsc::Sender<SynchronizerEvent>,
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
pub enum SynchronizerError {}
pub enum SynchronizerCommand {}
pub enum SynchronizerEvent {}
