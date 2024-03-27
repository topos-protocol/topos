use std::time::Duration;

use futures::{future::join_all, FutureExt};
use rstest::rstest;
use test_log::test;
use topos_test_sdk::tce::NodeConfig;
use tracing::Instrument;

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn two_bootnode_communicating() {
    let bootnode = NodeConfig::memory(2);
    let local = NodeConfig::memory(1);
    let bootnode_known_peers = vec![(local.peer_id(), local.addr.clone())];
    let local_known_peers = vec![(bootnode.peer_id(), bootnode.addr.clone())];

    let mut handlers = Vec::new();

    let context_local = tracing::info_span!("start_node", "peer_id" = local.peer_id().to_string());

    let context_bootnode =
        tracing::info_span!("start_node", "peer_id" = bootnode.peer_id().to_string());
    handlers.push(
        async move {
            let (client, mut stream, runtime) = crate::network::builder()
                .minimum_cluster_size(1)
                .peer_key(local.keypair.clone())
                .listen_addresses(&[local.addr.clone()])
                .known_peers(&local_known_peers)
                .memory()
                .build()
                .await
                .expect("Unable to create p2p network");

            runtime.bootstrap(&mut stream).await
        }
        .instrument(context_local)
        .boxed(),
    );

    handlers.push(
        async move {
            let (client, mut stream, runtime) = crate::network::builder()
                .minimum_cluster_size(1)
                .peer_key(bootnode.keypair.clone())
                .listen_addresses(&[bootnode.addr.clone()])
                .known_peers(&bootnode_known_peers)
                .memory()
                .build()
                .await
                .expect("Unable to create p2p network");

            runtime.bootstrap(&mut stream).await
        }
        .instrument(context_bootnode)
        .boxed(),
    );
    assert!(join_all(handlers).await.iter().all(Result::is_ok));
}
