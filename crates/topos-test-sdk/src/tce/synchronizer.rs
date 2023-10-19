use futures::Stream;
use std::future::IntoFuture;
use std::sync::Arc;
use tokio::{spawn, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use topos_p2p::NetworkClient;
use topos_tce_gatekeeper::GatekeeperClient;
use topos_tce_storage::validator::ValidatorStore;
use topos_tce_synchronizer::SynchronizerError;
use topos_tce_synchronizer::SynchronizerEvent;

pub async fn create_synchronizer(
    gatekeeper_client: GatekeeperClient,
    network_client: NetworkClient,
    store: Arc<ValidatorStore>,
) -> (
    impl Stream<Item = SynchronizerEvent>,
    JoinHandle<Result<(), SynchronizerError>>,
) {
    let shutdown = CancellationToken::new();
    let (synchronizer_runtime, synchronizer_stream) =
        topos_tce_synchronizer::Synchronizer::builder()
            .with_shutdown(shutdown)
            .with_store(store)
            .with_gatekeeper_client(gatekeeper_client)
            .with_network_client(network_client)
            .build()
            .expect("Can't create the Synchronizer");

    let synchronizer_join_handle = spawn(synchronizer_runtime.into_future());

    (synchronizer_stream, synchronizer_join_handle)
}
