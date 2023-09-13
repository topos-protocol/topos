//!
//! Application logic glue
//!
use crate::events::Events;
use futures::{Stream, StreamExt};
use prometheus::HistogramTimer;
use std::collections::HashMap;
use std::sync::Arc;
use tce_transport::ProtocolEvents;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use topos_core::uci::CertificateId;
use topos_metrics::CERTIFICATE_DELIVERED_TOTAL;
use topos_p2p::{Client as NetworkClient, Event as NetEvent};
use topos_tce_api::RuntimeClient as ApiClient;
use topos_tce_api::RuntimeEvent as ApiEvent;
use topos_tce_broadcast::ReliableBroadcastClient;
use topos_tce_gatekeeper::Client as GatekeeperClient;
use topos_tce_storage::authority::AuthorityStore;
use topos_tce_storage::events::StorageEvent;
use topos_tce_storage::types::CertificateDeliveredWithPositions;
use topos_tce_storage::StorageClient;
use topos_tce_synchronizer::{SynchronizerClient, SynchronizerEvent};
use tracing::{error, info, warn};

mod api;
mod network;
mod protocol;

/// Top-level transducer main app context & driver (alike)
///
/// Implements <...Host> traits for network and Api, listens for protocol events in events
/// (store is not active component).
///
/// In the end we shall come to design where this struct receives
/// config+data as input and runs app returning data as output
///
pub struct AppContext {
    pub events: mpsc::Sender<Events>,
    pub tce_cli: ReliableBroadcastClient,
    pub network_client: NetworkClient,
    pub api_client: ApiClient,
    pub pending_storage: StorageClient,
    pub gatekeeper: GatekeeperClient,
    pub synchronizer: SynchronizerClient,

    pub delivery_latency: HashMap<CertificateId, HistogramTimer>,

    pub authority_store: Arc<AuthorityStore>,
}

impl AppContext {
    // Default previous certificate id for first certificate in the subnet
    // TODO: Remove, it will be genesis certificate id retrieved from Topos Subnet
    const DUMMY_INITIAL_CERTIFICATE_ID: CertificateId =
        CertificateId::from_array([0u8; topos_core::uci::CERTIFICATE_ID_LENGTH]);

    /// Factory
    pub fn new(
        pending_storage: StorageClient,
        tce_cli: ReliableBroadcastClient,
        network_client: NetworkClient,
        api_client: ApiClient,
        gatekeeper: GatekeeperClient,
        synchronizer: SynchronizerClient,
        authority_store: Arc<AuthorityStore>,
    ) -> (Self, mpsc::Receiver<Events>) {
        let (events, receiver) = mpsc::channel(100);
        (
            Self {
                events,
                tce_cli,
                network_client,
                api_client,
                pending_storage,
                gatekeeper,
                synchronizer,
                delivery_latency: Default::default(),
                authority_store,
            },
            receiver,
        )
    }

    /// Main processing loop
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        mut self,
        mut network_stream: impl Stream<Item = NetEvent> + Unpin,
        mut tce_stream: impl Stream<Item = ProtocolEvents> + Unpin,
        mut api_stream: impl Stream<Item = ApiEvent> + Unpin,
        mut storage_stream: impl Stream<Item = StorageEvent> + Unpin,
        mut synchronizer_stream: impl Stream<Item = SynchronizerEvent> + Unpin,
        mut broadcast_stream: impl Stream<Item = CertificateDeliveredWithPositions> + Unpin,
        shutdown: (CancellationToken, mpsc::Sender<()>),
    ) {
        loop {
            tokio::select! {

                Some(delivery) = broadcast_stream.next() => {
                    let certificate_id = delivery.0.certificate.id;
                    CERTIFICATE_DELIVERED_TOTAL.inc();

                    if let Some(timer) = self.delivery_latency.remove(&certificate_id) {
                        let duration = timer.stop_and_record();
                        warn!("Certificate delivered {} in {}s", certificate_id, duration);
                    }
                }

                // protocol
                Some(evt) = tce_stream.next() => {
                    self.on_protocol_event(evt).await;
                },

                // network
                Some(net_evt) = network_stream.next() => {
                    self.on_net_event(net_evt).await;
                }

                // api events
                Some(event) = api_stream.next() => {
                    self.on_api_event(event).await;
                }

                // Storage events
                Some(_event) = storage_stream.next() => {
                }

                // Synchronizer events
                Some(_event) = synchronizer_stream.next() => {
                }

                // Shutdown signal
                _ = shutdown.0.cancelled() => {
                    info!("Shutting down TCE app context...");
                    if let Err(e) = self.shutdown().await {
                        error!("Error shutting down TCE app context: {e}");
                    }
                    // Drop the sender to notify the TCE termination
                    drop(shutdown.1);
                    break;
                }
            }
        }
        warn!("Exiting main TCE app processing loop")
    }

    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Shutting down the TCE client...");
        self.api_client.shutdown().await?;
        self.synchronizer.shutdown().await?;
        self.pending_storage.shutdown().await?;
        self.tce_cli.shutdown().await?;
        self.gatekeeper.shutdown().await?;
        self.network_client.shutdown().await?;

        Ok(())
    }
}
