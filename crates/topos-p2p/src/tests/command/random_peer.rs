use std::time::Duration;

use rstest::rstest;
use test_log::test;
use tokio::spawn;
use topos_test_sdk::tce::NodeConfig;

use crate::error::P2PError;

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn no_random_peer() {
    let local = NodeConfig::from_seed(1);

    let (client, _, runtime) = crate::network::builder()
        .peer_key(local.keypair.clone())
        .advertised_addresses(vec![local.addr.clone()])
        .listen_addresses(vec![local.addr.clone()])
        .build()
        .await
        .expect("Unable to create p2p network");

    let mut runtime = runtime.bootstrap().await.unwrap();

    spawn(runtime.run());

    let result = client.random_known_peer().await;

    assert!(result.is_err());
    assert!(matches!(
        result,
        Err(P2PError::CommandError(
            crate::error::CommandExecutionError::NoKnownPeer
        ))
    ));
}

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn return_a_peer() {
    let local = NodeConfig::from_seed(1);
    let expected = NodeConfig::from_seed(2);
    let expected_peer_id = expected.keypair.public().to_peer_id();

    let (client, _, runtime) = crate::network::builder()
        .peer_key(local.keypair.clone())
        .advertised_addresses(vec![local.addr.clone()])
        .listen_addresses(vec![local.addr.clone()])
        .build()
        .await
        .expect("Unable to create p2p network");

    let mut runtime = runtime.bootstrap().await.unwrap();

    runtime.peer_set.insert(expected_peer_id);

    spawn(runtime.run());

    let result = client.random_known_peer().await;

    assert!(result.is_ok());
    assert!(matches!(
        result,
        Ok(peer) if peer == expected_peer_id
    ));
}

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn return_a_random_peer_among_100() {
    let local = NodeConfig::from_seed(1);

    let (client, _, runtime) = crate::network::builder()
        .peer_key(local.keypair.clone())
        .advertised_addresses(vec![local.addr.clone()])
        .listen_addresses(vec![local.addr.clone()])
        .build()
        .await
        .expect("Unable to create p2p network");

    let mut runtime = runtime.bootstrap().await.unwrap();

    for i in 2..=100 {
        let peer = NodeConfig::from_seed(i);
        runtime.peer_set.insert(peer.keypair.public().to_peer_id());
    }

    spawn(runtime.run());

    let first_try = client.random_known_peer().await.unwrap();
    let second_try = client.random_known_peer().await.unwrap();
    let third_try = client.random_known_peer().await.unwrap();

    assert!(first_try != second_try);
    assert!(first_try != third_try);
}
