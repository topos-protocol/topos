use std::{collections::HashMap, time::Duration};

use futures::{future::join_all, Stream, StreamExt};
use libp2p::{Multiaddr, PeerId};

use topos_core::api::tce::v1::{PushPeerListRequest, StatusRequest};
use topos_p2p::Client;
use topos_test_sdk::tce::{NodeConfig, TceContext};
use tracing::{info, info_span, Instrument};

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

            (client.peer_id, client)
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
    (1..=peer_number).map(NodeConfig::from_seed).collect()
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

pub fn sample_lower_bound(n_u: usize) -> usize {
    let k: f32 = 2.;
    (n_u as f32).log(k) as usize
}

pub(crate) async fn create_network(
    peer_number: usize,
    correct_sample: usize,
) -> HashMap<PeerId, TceContext> {
    let g = |a, b: f32| ((a as f32) * b).ceil() as usize;

    // List of peers (tce nodes) with their context
    let mut peers_context = start_peer_pool(peer_number as u8, correct_sample, g).await;
    let all_peers: Vec<PeerId> = peers_context.keys().cloned().collect();

    // Force TCE nodes to recreate subscriptions and subscribers
    info!("Trigger the new network view");
    for (peer_id, client) in peers_context.iter_mut() {
        let _ = client
            .console_grpc_client
            .push_peer_list(PushPeerListRequest {
                request_id: None,
                peers: all_peers
                    .iter()
                    .filter_map(|key| {
                        if key == peer_id {
                            None
                        } else {
                            Some(key.to_string())
                        }
                    })
                    .collect::<Vec<_>>(),
            })
            .await
            .expect("Can't send PushPeerListRequest");
    }

    tokio::time::sleep(Duration::from_secs(2)).await;

    // Waiting for new network view
    let mut status: Vec<bool> = Vec::new();

    for (_peer_id, client) in peers_context.iter_mut() {
        let response = client
            .console_grpc_client
            .status(StatusRequest {})
            .await
            .expect("Can't get status");

        status.push(response.into_inner().has_active_sample);
    }

    assert!(status.iter().all(|s| *s));

    peers_context
}
