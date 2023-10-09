use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use topos_p2p::PeerId;

use crate::{client::GatekeeperClient, Gatekeeper, GatekeeperError};

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

    /// Start the Gatekeeper with a set of known peers in the network
    /// The Sync service for example is making use of this list to ask for
    /// random peers
    pub fn peer_list(mut self, peer_list: Vec<PeerId>) -> Self {
        self.local_peer_list = Some(peer_list);

        self
    }
}

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
                peer_list: self.local_peer_list.unwrap_or_default(),
                commands: commands_recv,
                ..Gatekeeper::default()
            },
        ))
        .boxed()
    }
}
