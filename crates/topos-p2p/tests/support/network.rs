use std::net::UdpSocket;

use futures::{Stream, StreamExt};
use libp2p::{
    identity::{ed25519::SecretKey, Keypair},
    Multiaddr, PeerId,
};
use tokio::spawn;
use topos_p2p::{network, Client};

pub fn local_peer(peer_index: u8) -> (Keypair, Multiaddr) {
    let peer_id: Keypair = keypair_from_byte(peer_index);
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let port = socket.local_addr().unwrap().port();
    let local_listen_addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}", port).parse().unwrap();
    (peer_id, local_listen_addr)
}

pub fn local_peers(count: u8) -> Vec<(Keypair, Multiaddr)> {
    (0..count).map(|i: u8| local_peer(i)).collect()
}

pub async fn start_node(
    (peer_key, peer_addr): (Keypair, Multiaddr),
    known_peers: Vec<(PeerId, Multiaddr)>,
) -> TestNodeContext {
    let peer_id = peer_key.public().to_peer_id();

    let (client, event_stream, event_loop) = network::builder()
        .peer_key(peer_key)
        .known_peers(known_peers)
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

pub fn keypair_from_byte(seed: u8) -> Keypair {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;
    let secret_key = SecretKey::from_bytes(&mut bytes)
        .expect("this returns `Err` only if the length is wrong but len is fixed in this context");
    Keypair::Ed25519(secret_key.into())
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
