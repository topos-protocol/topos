use std::{future::IntoFuture, sync::Arc};

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use topos_p2p::NetworkClient;
use topos_tce_gatekeeper::GatekeeperClient;
use topos_tce_storage::validator::ValidatorStore;

use super::{
    CheckpointSynchronizer, CheckpointsCollectorClient, CheckpointsCollectorConfig,
    CheckpointsCollectorError, CheckpointsCollectorEvent,
};

pub struct CheckpointsCollectorBuilder<G: GatekeeperClient, N: NetworkClient> {
    gatekeeper_client: Option<G>,
    network_client: Option<N>,
    store: Option<Arc<ValidatorStore>>,
    sync_interval_seconds: u64,
}

impl<G: GatekeeperClient, N: NetworkClient> Default for CheckpointsCollectorBuilder<G, N> {
    fn default() -> Self {
        Self {
            gatekeeper_client: None,
            network_client: None,
            store: None,
            sync_interval_seconds: 10,
        }
    }
}
impl<G: GatekeeperClient, N: NetworkClient> CheckpointsCollectorBuilder<G, N> {
    pub fn set_gatekeeper_client(mut self, gatekeeper_client: Option<G>) -> Self {
        self.gatekeeper_client = gatekeeper_client;

        self
    }

    pub fn set_network_client(mut self, network_client: Option<N>) -> Self {
        self.network_client = network_client;

        self
    }

    pub fn set_sync_interval_seconds(mut self, sync_interval_seconds: u64) -> Self {
        self.sync_interval_seconds = sync_interval_seconds;

        self
    }

    pub fn set_store(mut self, store: Option<Arc<ValidatorStore>>) -> Self {
        self.store = store;

        self
    }
}

impl<G: GatekeeperClient, N: NetworkClient> IntoFuture for CheckpointsCollectorBuilder<G, N> {
    type Output = Result<
        (
            CheckpointsCollectorClient,
            CheckpointSynchronizer<G, N>,
            ReceiverStream<CheckpointsCollectorEvent>,
        ),
        CheckpointsCollectorError,
    >;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel(1);
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

        let store = if let Some(store) = self.store {
            store
        } else {
            return futures::future::err(CheckpointsCollectorError::NoStore).boxed();
        };
        futures::future::ok((
            CheckpointsCollectorClient { shutdown_channel },
            CheckpointSynchronizer {
                config: CheckpointsCollectorConfig::default(),
                network,
                store,
                current_request_id: None,
                shutdown,
                events,
                gatekeeper,
            },
            ReceiverStream::new(events_recv),
        ))
        .boxed()
    }
}
