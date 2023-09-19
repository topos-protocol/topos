use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use topos_p2p::PeerId;

use crate::{client::Client, Gatekeeper, GatekeeperError};

#[derive(Default)]
pub struct GatekeeperBuilder {
    local_peer_id: Option<PeerId>,
}

impl GatekeeperBuilder {
    pub fn local_peer_id(mut self, peer_id: PeerId) -> Self {
        self.local_peer_id = Some(peer_id);

        self
    }
}

impl IntoFuture for GatekeeperBuilder {
    type Output = Result<(Client, Gatekeeper), GatekeeperError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
        let (commands, commands_recv) = mpsc::channel(100);

        // TODO: Fix this unwrap later
        let local_peer_id = self.local_peer_id.take().unwrap();

        futures::future::ok((
            Client {
                shutdown_channel,
                commands,
                local_peer_id,
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
