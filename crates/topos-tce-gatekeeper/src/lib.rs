use std::{future::IntoFuture, time::Duration};

use builder::GatekeeperBuilder;
use futures::{future::BoxFuture, FutureExt};
use rand::seq::SliceRandom;
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot},
    time,
};
use tracing::error;

mod builder;
mod client;
#[cfg(test)]
mod tests;

pub use client::GatekeeperClient;
use topos_core::uci::SubnetId;
use tracing::{info, warn};

pub struct Gatekeeper {
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,
    pub(crate) tick_duration: Duration,

    subnet_list: Vec<SubnetId>,
}

impl Default for Gatekeeper {
    fn default() -> Self {
        let (_shutdown_channel, shutdown) = mpsc::channel(1);
        let tick_duration = Duration::from_secs(Self::DEFAULT_TICK_DURATION);

        Self {
            shutdown,
            tick_duration,
            subnet_list: Vec::default(),
        }
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
                }
            };

            if let Some(sender) = shutdowned {
                info!("Shutting down gatekeeper...");
                _ = sender.send(());
            } else {
                warn!("Shutting down gatekeeper due to error...");
            }

            Ok(())
        }
        .boxed()
    }
}

impl Gatekeeper {
    pub(crate) const DEFAULT_TICK_DURATION: u64 = 10;

    pub fn builder() -> GatekeeperBuilder {
        GatekeeperBuilder::default()
    }
}

#[derive(Debug, Error)]
pub enum GatekeeperError {
    #[error("Unable to receive expected response from Gatekeeper: {0}")]
    ResponseChannel(#[from] oneshot::error::RecvError),

    #[error("Unable to execute command on the Gatekeeper: {0}")]
    InvalidCommand(String),

    #[error("Unable to execute shutdown on the Gatekeeper: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),

    #[error("The command produce no update")]
    NoUpdate,
}
