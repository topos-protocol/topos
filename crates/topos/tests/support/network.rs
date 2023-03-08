use std::collections::HashMap;

use futures::{future::join_all, Stream, StreamExt};
use libp2p::{Multiaddr, PeerId};

use topos_p2p::Client;
use topos_test_sdk::tce::{NodeConfig, TceContext};
use tracing::{info_span, Instrument};

pub async fn start_peer_pool(
    peer_number: u8,
    correct_sample: usize,
    g: fn(usize, f32) -> usize,
) -> HashMap<PeerId, TceContext> {
    let mut clients = HashMap::new();
    let peers = build_peer_config_pool(peer_number);

    let mut await_peers = Vec::new();
    for config in &peers {
        let mut config = config.clone();
        config.correct_sample = correct_sample;
        config.g = g;

        let span = info_span!(
            "Peer",
            peer = config.keypair.public().to_peer_id().to_string()
        );

        let fut = async {
            let client = topos_test_sdk::tce::start_node(vec![], config, &peers).await;

            (client.peer_id.clone(), client)
        }
        .instrument(span);

        await_peers.push(fut);
    }

    for (user_peer_id, client) in join_all(await_peers).await {
        clients.insert(user_peer_id, client);
    }

    clients
}

fn build_peer_config_pool(peer_number: u8) -> Vec<NodeConfig> {
    (1..=peer_number)
        .into_iter()
        .map(|id| NodeConfig::from_seed(id))
        .collect()
}

#[allow(dead_code)]
pub struct TestNodeContext {
    pub(crate) peer_id: PeerId,
    pub(crate) peer_addr: Multiaddr,
    pub(crate) client: Client,
    stream: Box<dyn Stream<Item = topos_p2p::Event> + Unpin + Send>,
}

impl TestNodeContext {
    #[allow(dead_code)]
    pub(crate) async fn next_event(&mut self) -> Option<topos_p2p::Event> {
        self.stream.next().await
    }
}
