use std::{future::IntoFuture, sync::Arc};

use futures::{future::BoxFuture, FutureExt, TryFutureExt};
use tokio::{spawn, sync::mpsc, sync::oneshot};
use tokio_stream::wrappers::ReceiverStream;
use topos_p2p::Client as NetworkClient;
use topos_tce_gatekeeper::Client as GatekeeperClient;
use topos_tce_storage::validator::ValidatorStore;

use crate::{
    checkpoints_collector::CheckpointSynchronizer, client::SynchronizerClient, Synchronizer,
    SynchronizerError, SynchronizerEvent,
};

pub struct SynchronizerBuilder {
    gatekeeper_client: Option<GatekeeperClient>,
    network_client: Option<NetworkClient>,
    store: Option<Arc<ValidatorStore>>,
    sync_interval_seconds: u64,
}

impl Default for SynchronizerBuilder {
    fn default() -> Self {
        Self {
            gatekeeper_client: None,
            network_client: None,
            store: None,
            sync_interval_seconds: 1,
        }
    }
}

impl IntoFuture for SynchronizerBuilder {
    type Output = Result<
        (
            SynchronizerClient,
            Synchronizer,
            ReceiverStream<SynchronizerEvent>,
        ),
        SynchronizerError,
    >;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        let (shutdown_channel, shutdown) = mpsc::channel::<oneshot::Sender<()>>(1);
        let (events, events_recv) = mpsc::channel(100);

        CheckpointSynchronizer::builder()
            .set_gatekeeper_client(self.gatekeeper_client.take())
            .set_network_client(self.network_client.take())
            .set_sync_interval_seconds(self.sync_interval_seconds)
            .set_store(self.store)
            .into_future()
            .map_err(Into::into)
            .and_then(
                |(checkpoints_collector, runtime, checkpoints_collector_stream)| {
                    spawn(runtime.into_future());

                    futures::future::ok((
                        SynchronizerClient { shutdown_channel },
                        Synchronizer {
                            shutdown,
                            events,

                            checkpoints_collector,
                            checkpoints_collector_stream,
                        },
                        ReceiverStream::new(events_recv),
                    ))
                },
            )
            .boxed()
    }
}

impl SynchronizerBuilder {
    pub fn with_store(mut self, store: Arc<ValidatorStore>) -> Self {
        self.store = Some(store);

        self
    }

    pub fn with_gatekeeper_client(mut self, gatekeeper_client: GatekeeperClient) -> Self {
        self.gatekeeper_client = Some(gatekeeper_client);

        self
    }

    pub fn with_network_client(mut self, network_client: NetworkClient) -> Self {
        self.network_client = Some(network_client);

        self
    }

    pub fn with_sync_interval_seconds(mut self, sync_interval_seconds: u64) -> Self {
        self.sync_interval_seconds = sync_interval_seconds;

        self
    }
}
