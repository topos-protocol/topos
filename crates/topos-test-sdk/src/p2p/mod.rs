use libp2p::{
    identity::{self, Keypair},
    Multiaddr,
};

use crate::networking::get_available_port;

pub type Port = u16;

pub fn local_peer(peer_index: u8) -> (Keypair, Port, Multiaddr) {
    let peer_id: Keypair = keypair_from_seed(peer_index);
    let port = get_available_port();
    let local_listen_addr: Multiaddr = format!(
        "/ip4/127.0.0.1/tcp/{}/p2p/{}",
        port,
        peer_id.public().to_peer_id()
    )
    .parse()
    .unwrap();

    (peer_id, port, local_listen_addr)
}

pub fn keypair_from_seed(seed: u8) -> Keypair {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;

    identity::Keypair::ed25519_from_bytes(bytes).expect("Invalid keypair")
}
