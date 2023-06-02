#![allow(unused_variables)]
#![allow(unused_imports)]
mod behaviour;
mod client;
mod command;
pub mod config;
mod constant;
pub mod error;
mod event;
mod runtime;
#[cfg(test)]
mod tests;

pub(crate) use behaviour::Behaviour;
pub use client::Client;
pub use client::RetryPolicy;
pub(crate) use command::Command;
pub use command::NotReadyMessage;
pub use event::Event;
pub use libp2p::Multiaddr;
pub use libp2p::PeerId;
pub use runtime::Runtime;

pub mod network;

pub const TOPOS_GOSSIP: &str = "topos_gossip";
pub const TOPOS_ECHO: &str = "topos_echo";
pub const TOPOS_READY: &str = "topos_ready";

pub mod utils {
    use libp2p::identity;

    /// build peer_id keys, generate for now - either from the seed or purely random one
    pub fn local_key_pair(secret_key_seed: Option<u8>) -> identity::Keypair {
        // todo: load from protobuf encoded|base64 encoded config.local_key_pair
        match secret_key_seed {
            Some(seed) => {
                let mut bytes = [0u8; 32];
                bytes[0] = seed;
                identity::Keypair::ed25519_from_bytes(bytes).expect("Invalid keypair")
            }
            None => identity::Keypair::generate_ed25519(),
        }
    }

    pub fn local_key_pair_from_slice(slice: &[u8]) -> identity::Keypair {
        // todo: load from protobuf encoded|base64 encoded config.local_key_pair
        let mut bytes = [0u8; 32];
        if slice.len() <= 32 {
            bytes[..slice.len()].clone_from_slice(slice);
        } else {
            bytes.clone_from_slice(&slice[..32]);
        }

        identity::Keypair::ed25519_from_bytes(bytes).expect("Invalid keypair")
    }
}
