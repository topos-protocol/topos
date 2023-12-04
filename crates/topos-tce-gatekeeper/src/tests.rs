use std::future::IntoFuture;

use rstest::{fixture, rstest};
use test_log::test;
use tokio::spawn;
use topos_p2p::PeerId;

use crate::{client::GatekeeperClient, Gatekeeper};

#[test(tokio::test)]
async fn can_start_and_stop() -> Result<(), Box<dyn std::error::Error>> {
    let (client, server) = Gatekeeper::builder().await?;

    let handler = spawn(server.into_future());

    client.shutdown().await?;

    assert!(handler.is_finished());

    Ok(())
}

#[fixture]
async fn gatekeeper() -> GatekeeperClient {
    let (client, server) = Gatekeeper::builder().await.unwrap();

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
