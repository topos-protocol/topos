use futures::{Stream, StreamExt};
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use tokio::spawn;
use topos_p2p::{network, Client};

pub use topos_test_sdk::p2p::local_peer;

pub fn local_peers(count: u8) -> Vec<(Keypair, u16, Multiaddr)> {
    (0..count).map(|i: u8| local_peer(i)).collect()
}

pub async fn start_node(
    (peer_key, _, peer_addr): (Keypair, u16, Multiaddr),
    known_peers: Vec<(PeerId, Multiaddr)>,
) -> TestNodeContext {
    let peer_id = peer_key.public().to_peer_id();

    let (client, event_stream, event_loop) = network::builder()
        .peer_key(peer_key)
        .known_peers(&known_peers)
        .listen_addr(peer_addr.clone())
        .exposed_addresses(peer_addr.clone())
        .build()
        .await
        .unwrap();

    spawn(event_loop.run());

    let _ = client.start_listening(peer_addr.clone()).await;

    TestNodeContext {
        peer_id,
        peer_addr,
        client,
        stream: Box::new(event_stream),
    }
}

pub struct TestNodeContext {
    pub(crate) peer_id: PeerId,
    pub(crate) peer_addr: Multiaddr,
    pub(crate) client: Client,
    stream: Box<dyn Stream<Item = topos_p2p::Event> + Unpin + Send>,
}

impl TestNodeContext {
    pub(crate) async fn next_event(&mut self) -> Option<topos_p2p::Event> {
        self.stream.next().await
    }
}
