use std::{future::IntoFuture, time::Duration};

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;

use crate::{client::GatekeeperClient, Gatekeeper, GatekeeperError};

const DEFAULT_TICK_DURATION: u64 = 10;

pub struct GatekeeperBuilder {}

impl IntoFuture for GatekeeperBuilder {
    type Output = Result<(GatekeeperClient, Gatekeeper), GatekeeperError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
        let (commands, commands_recv) = mpsc::channel(100);
        let tick_duration = Duration::from_secs(DEFAULT_TICK_DURATION);

        futures::future::ok((
            GatekeeperClient {
                shutdown_channel,
                commands,
            },
            Gatekeeper {
                shutdown,
                tick_duration,
                commands: commands_recv,
            },
        ))
        .boxed()
    }
}
