use std::{future::IntoFuture, time::Duration};

use builder::GatekeeperBuilder;
use futures::{future::BoxFuture, FutureExt};
use tokio::{sync::mpsc, time};

mod builder;
mod client;

pub use client::GatekeeperClient;

pub struct Gatekeeper {
    pub(crate) shutdown: mpsc::Receiver<()>,
    pub(crate) commands: mpsc::Receiver<GatekeeperCommand>,
    pub(crate) tick_duration: Duration,
}

impl IntoFuture for Gatekeeper {
    type Output = Result<(), GatekeeperError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            let mut interval = time::interval(self.tick_duration);

            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    _ = self.shutdown.recv() => { break; }
                    _command = self.commands.recv() => {}
                }
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

#[derive(Debug)]
pub enum GatekeeperError {}
pub enum GatekeeperCommand {}
