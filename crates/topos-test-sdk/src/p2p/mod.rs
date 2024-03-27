use libp2p::{
    build_multiaddr,
    identity::{self, Keypair},
    Multiaddr,
};
use rand::{thread_rng, Rng};

use crate::networking::get_available_port;

pub fn local_peer(peer_index: u8, memory_transport: bool) -> (Keypair, Multiaddr) {
    let peer_id: Keypair = keypair_from_seed(peer_index);
    let local_listen_addr = if memory_transport {
        build_multiaddr![Memory(thread_rng().gen::<u64>())]
    } else {
        let port = get_available_port();
        format!(
            "/ip4/127.0.0.1/tcp/{}/p2p/{}",
            port,
            peer_id.public().to_peer_id()
        )
        .parse()
        .unwrap()
    };

    (peer_id, local_listen_addr)
}

pub fn keypair_from_seed(seed: u8) -> Keypair {
    let mut bytes = [0u8; 32];
    bytes[0] = seed;

    identity::Keypair::ed25519_from_bytes(bytes).expect("Invalid keypair")
}
