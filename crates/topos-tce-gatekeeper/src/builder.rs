use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use topos_p2p::PeerId;

use crate::{client::GatekeeperClient, Gatekeeper, GatekeeperError};

#[derive(Default)]
pub struct GatekeeperBuilder {}

impl IntoFuture for GatekeeperBuilder {
    type Output = Result<(GatekeeperClient, Gatekeeper), GatekeeperError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
        let (commands, commands_recv) = mpsc::channel(100);

        futures::future::ok((
            GatekeeperClient {
                shutdown_channel,
                commands,
            },
            Gatekeeper {
                shutdown,
                commands: commands_recv,
                ..Gatekeeper::default()
            },
        ))
        .boxed()
    }
}
