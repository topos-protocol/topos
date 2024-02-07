use std::borrow::Cow;

use crate::{config::DiscoveryConfig, error::CommandExecutionError};
use libp2p::kad::Event;
use libp2p::{
    identity::Keypair,
    kad::{store::MemoryStore, Behaviour, BucketInserts, Config},
    swarm::NetworkBehaviour,
    Multiaddr, PeerId,
};
use tokio::sync::oneshot;
use tracing::info;

pub type PendingRecordRequest = oneshot::Sender<Result<Vec<Multiaddr>, CommandExecutionError>>;

/// DiscoveryBehaviour is responsible to discover and manage connections with peers
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "Event")]
pub(crate) struct DiscoveryBehaviour {
    pub(crate) inner: Behaviour<MemoryStore>,
}

impl DiscoveryBehaviour {
    pub fn create(
        config: &DiscoveryConfig,
        peer_key: Keypair,
        discovery_protocol: Cow<'static, [u8]>,
        known_peers: &[(PeerId, Multiaddr)],
        _with_mdns: bool,
    ) -> Self {
        let local_peer_id = peer_key.public().to_peer_id();
        let kademlia_config = Config::default()
            .set_replication_factor(config.replication_factor)
            .set_kbucket_inserts(BucketInserts::Manual)
            .set_replication_interval(config.replication_interval)
            .set_publication_interval(config.publication_interval)
            .set_provider_publication_interval(config.provider_publication_interval)
            .to_owned();

        let mut kademlia = Behaviour::with_config(
            local_peer_id,
            MemoryStore::new(local_peer_id),
            kademlia_config,
        );

        for known_peer in known_peers {
            info!(
                "Adding the known peer:{} reachable at {}",
                &known_peer.0, &known_peer.1
            );
            kademlia.add_address(&known_peer.0, known_peer.1.clone());
        }

        Self { inner: kademlia }
    }

    pub fn get_addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
        if let Some(key_ref) = self.inner.kbucket(*peer_id) {
            key_ref
                .iter()
                .filter(|e| e.node.key.preimage() == peer_id)
                .map(|e| e.node.value.first().clone())
                .collect()
        } else {
            Vec::new()
        }
    }
}
