use std::future::IntoFuture;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use topos_p2p::Client as NetworkClient;
use topos_tce_gatekeeper::GatekeeperClient;

use super::{
    CheckpointsCollector, CheckpointsCollectorClient, CheckpointsCollectorConfig,
    CheckpointsCollectorError, CheckpointsCollectorEvent,
};

#[derive(Default)]
pub struct CheckpointsCollectorBuilder {
    gatekeeper_client: Option<GatekeeperClient>,
    network_client: Option<NetworkClient>,
}

impl CheckpointsCollectorBuilder {
    pub fn set_gatekeeper_client(mut self, gatekeeper_client: Option<GatekeeperClient>) -> Self {
        self.gatekeeper_client = gatekeeper_client;

        self
    }
}

impl IntoFuture for CheckpointsCollectorBuilder {
    type Output = Result<
        (
            CheckpointsCollectorClient,
            CheckpointsCollector,
            ReceiverStream<CheckpointsCollectorEvent>,
        ),
        CheckpointsCollectorError,
    >;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
        let (commands, commands_recv) = mpsc::channel(100);
        let (events, events_recv) = mpsc::channel(100);

        let gatekeeper = if let Some(gatekeeper) = self.gatekeeper_client {
            gatekeeper
        } else {
            return futures::future::err(CheckpointsCollectorError::NoGatekeeperClient).boxed();
        };

        let network = if let Some(network) = self.network_client {
            network
        } else {
            return futures::future::err(CheckpointsCollectorError::NoNetworkClient).boxed();
        };

        futures::future::ok((
            CheckpointsCollectorClient {
                shutdown_channel,
                commands,
            },
            CheckpointsCollector {
                started: false,
                config: CheckpointsCollectorConfig::default(),
                network,
                shutdown,
                commands: commands_recv,
                events,
                gatekeeper,
            },
            ReceiverStream::new(events_recv),
        ))
        .boxed()
    }
}
