use std::{future::IntoFuture, sync::Arc};

use tokio::{spawn, sync::mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use topos_p2p::NetworkClient;
use topos_tce_gatekeeper::GatekeeperClient;
use topos_tce_storage::validator::ValidatorStore;

use crate::{
    checkpoints_collector::{
        CheckpointSynchronizer, CheckpointsCollectorConfig, CheckpointsCollectorError,
    },
    Synchronizer, SynchronizerError, SynchronizerEvent,
};

pub struct SynchronizerBuilder {
    gatekeeper_client: Option<GatekeeperClient>,
    network_client: Option<NetworkClient>,
    store: Option<Arc<ValidatorStore>>,
    sync_interval_seconds: u64,
    /// Size of the channel producing events (default: 100)
    event_channel_size: usize,
    /// CancellationToken used to trigger shutdown of the Synchronizer
    shutdown: Option<CancellationToken>,
}

impl Default for SynchronizerBuilder {
    fn default() -> Self {
        Self {
            gatekeeper_client: None,
            network_client: None,
            store: None,
            sync_interval_seconds: 1,
            event_channel_size: 100,
            shutdown: None,
        }
    }
}

impl SynchronizerBuilder {
    pub fn build(
        mut self,
    ) -> Result<(Synchronizer, ReceiverStream<SynchronizerEvent>), SynchronizerError> {
        let shutdown = if let Some(shutdown) = self.shutdown.take() {
            shutdown
        } else {
            return Err(SynchronizerError::CheckpointsCollectorError(
                CheckpointsCollectorError::NoStore,
            ))?;
        };
        let (events, events_recv) = mpsc::channel(self.event_channel_size);
        let (sync_events, checkpoints_collector_stream) = mpsc::channel(self.event_channel_size);

        let checkpoints_collector_stream = ReceiverStream::new(checkpoints_collector_stream);

        spawn(
            CheckpointSynchronizer {
                config: CheckpointsCollectorConfig::default(),
                network: if let Some(network) = self.network_client {
                    network
                } else {
                    return Err(SynchronizerError::CheckpointsCollectorError(
                        CheckpointsCollectorError::NoNetworkClient,
                    ))?;
                },
                gatekeeper: if let Some(gatekeeper) = self.gatekeeper_client {
                    gatekeeper
                } else {
                    return Err(SynchronizerError::CheckpointsCollectorError(
                        CheckpointsCollectorError::NoGatekeeperClient,
                    ))?;
                },
                store: if let Some(store) = self.store {
                    store
                } else {
                    return Err(SynchronizerError::CheckpointsCollectorError(
                        CheckpointsCollectorError::NoStore,
                    ))?;
                },
                current_request_id: None,
                shutdown: shutdown.child_token(),
                events: sync_events,
            }
            .into_future(),
        );

        Ok((
            Synchronizer {
                shutdown,
                events,
                checkpoints_collector_stream,
            },
            ReceiverStream::new(events_recv),
        ))
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

    pub fn with_shutdown(mut self, shutdown: CancellationToken) -> Self {
        self.shutdown = Some(shutdown);

        self
    }
}
