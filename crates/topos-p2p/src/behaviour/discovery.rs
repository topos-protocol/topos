use std::{borrow::Cow, collections::HashMap, num::NonZeroUsize, time::Duration};

use crate::{
    config::DiscoveryConfig,
    constant::TRANSMISSION_PROTOCOL,
    error::{CommandExecutionError, P2PError},
};
use libp2p::{
    identity::Keypair,
    kad::{store::MemoryStore, Kademlia, KademliaBucketInserts, KademliaConfig},
    swarm::{behaviour, NetworkBehaviour},
    Multiaddr, PeerId,
};
use libp2p::{kad::KademliaEvent, StreamProtocol};
use tokio::sync::oneshot;
use tracing::{debug, info, warn};

pub type PendingDials = HashMap<PeerId, oneshot::Sender<Result<(), P2PError>>>;
pub type PendingRecordRequest = oneshot::Sender<Result<Vec<Multiaddr>, CommandExecutionError>>;

/// DiscoveryBehaviour is responsible to discover and manage connections with peers
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "KademliaEvent")]
pub(crate) struct DiscoveryBehaviour {
    pub(crate) inner: Kademlia<MemoryStore>,
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
        let kademlia_config = KademliaConfig::default()
            .set_protocol_names(vec![StreamProtocol::new(TRANSMISSION_PROTOCOL)])
            .set_replication_factor(config.replication_factor)
            .set_kbucket_inserts(KademliaBucketInserts::Manual)
            .set_replication_interval(config.replication_interval)
            .set_publication_interval(config.publication_interval)
            .set_provider_publication_interval(config.provider_publication_interval)
            .to_owned();

        let mut kademlia = Kademlia::with_config(
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

        if let Err(store_error) = kademlia.start_providing("topos-tce".as_bytes().to_vec().into()) {
            warn!(reason = %store_error, "Could not start providing Kademlia protocol `topos-tce`")
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
