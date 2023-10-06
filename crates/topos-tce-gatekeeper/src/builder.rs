use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use topos_p2p::PeerId;

use crate::{client::Client, Gatekeeper, GatekeeperError};

#[derive(Default)]
pub struct GatekeeperBuilder {
    local_peer_id: Option<PeerId>,
    local_peer_list: Option<Vec<PeerId>>,
}

impl GatekeeperBuilder {
    pub fn local_peer_id(mut self, peer_id: PeerId) -> Self {
        self.local_peer_id = Some(peer_id);

        self
    }

    pub fn peer_list(mut self, peer_list: Vec<PeerId>) -> Self {
        self.local_peer_list = Some(peer_list);

        self
    }
}

impl IntoFuture for GatekeeperBuilder {
    type Output = Result<(Client, Gatekeeper), GatekeeperError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
        let (commands, commands_recv) = mpsc::channel(100);

        futures::future::ok((
            Client {
                shutdown_channel,
                commands,
            },
            Gatekeeper {
                shutdown,
                peer_list: self.local_peer_list.unwrap_or_default(),
                commands: commands_recv,
                ..Gatekeeper::default()
            },
        ))
        .boxed()
    }
}
