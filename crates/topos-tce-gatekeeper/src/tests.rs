use std::future::IntoFuture;

use rstest::{fixture, rstest};
use test_log::test;
use tokio::spawn;
use topos_p2p::PeerId;

use crate::{client::Client, Gatekeeper, GatekeeperClient};

#[test(tokio::test)]
async fn can_start_and_stop() -> Result<(), Box<dyn std::error::Error>> {
    let peer_id = topos_p2p::utils::local_key_pair(Some(99))
        .public()
        .to_peer_id();
    let (client, server) = Gatekeeper::builder().local_peer_id(peer_id).await?;

    let handler = spawn(server.into_future());

    client.shutdown().await?;

    assert!(handler.is_finished());

    Ok(())
}

#[rstest]
#[test(tokio::test)]
async fn can_fetch_full_or_partial_list(#[future] gatekeeper: Client) {
    let gatekeeper = gatekeeper.await;

    assert_eq!(10, gatekeeper.get_all_peers().await.unwrap().len());

    let first = gatekeeper.get_random_peers(5).await.unwrap();
    assert_eq!(5, first.len());

    let second = gatekeeper.get_random_peers(5).await.unwrap();
    assert_eq!(5, second.len());

    assert_ne!(first, second);
}

#[fixture]
async fn gatekeeper(peer_list: Vec<PeerId>) -> Client {
    let peer_id = topos_p2p::utils::local_key_pair(Some(99))
        .public()
        .to_peer_id();
    let (client, server) = Gatekeeper::builder()
        .local_peer_id(peer_id)
        .peer_list(peer_list)
        .await
        .unwrap();

    spawn(server.into_future());

    client
}

#[fixture]
fn peer_list(#[default(10)] number: usize) -> Vec<PeerId> {
    (0..number)
        .map(|i| {
            topos_p2p::utils::local_key_pair(Some(i as u8))
                .public()
                .to_peer_id()
        })
        .collect()
}
