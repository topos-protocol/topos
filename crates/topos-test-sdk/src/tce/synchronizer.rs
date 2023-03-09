use futures::Stream;
use std::future::IntoFuture;
use tokio::{spawn, task::JoinHandle};

use topos_p2p::Client;
use topos_tce_gatekeeper::GatekeeperClient;
use topos_tce_synchronizer::SynchronizerClient;
use topos_tce_synchronizer::SynchronizerError;
use topos_tce_synchronizer::SynchronizerEvent;

pub async fn create_synchronizer(
    gatekeeper_client: GatekeeperClient,
    network_client: Client,
) -> (
    SynchronizerClient,
    impl Stream<Item = SynchronizerEvent>,
    JoinHandle<Result<(), SynchronizerError>>,
) {
    let (synchronizer_client, synchronizer_runtime, synchronizer_stream) =
        topos_tce_synchronizer::Synchronizer::builder()
            .with_gatekeeper_client(gatekeeper_client.clone())
            .with_network_client(network_client.clone())
            .await
            .expect("Can't create the Synchronizer");

    let synchronizer_join_handle = spawn(synchronizer_runtime.into_future());

    (
        synchronizer_client,
        synchronizer_stream,
        synchronizer_join_handle,
    )
}
