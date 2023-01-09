use std::{borrow::Cow, collections::HashMap, num::NonZeroUsize};

use crate::error::{CommandExecutionError, P2PError};
use libp2p::{
    identity::Keypair,
    kad::{store::MemoryStore, Kademlia, KademliaConfig},
    Multiaddr, PeerId,
};
use tokio::sync::oneshot;
use tracing::{info, warn};

pub type PendingDials = HashMap<PeerId, oneshot::Sender<Result<(), P2PError>>>;
pub type PendingRecordRequest = oneshot::Sender<Result<Vec<Multiaddr>, CommandExecutionError>>;

/// DiscoveryBehaviour is responsible to discover and manage connections with peers
pub(crate) struct DiscoveryBehaviour {}
impl DiscoveryBehaviour {
    // #[instrument(name = "DiscoveryBehaviour", skip_all, fields(peer_id = %peer_key.public().to_peer_id()))]
    pub fn create(
        peer_key: Keypair,
        discovery_protocol: Cow<'static, [u8]>,
        known_peers: &[(PeerId, Multiaddr)],
        _with_mdns: bool,
    ) -> Kademlia<MemoryStore> {
        let local_peer_id = peer_key.public().to_peer_id();
        let kademlia_config = KademliaConfig::default()
            .set_protocol_names(vec![discovery_protocol])
            .set_replication_factor(NonZeroUsize::new(1).unwrap())
            // .set_replication_interval(Some(Duration::from_secs(30)))
            // .set_publication_interval(Some(Duration::from_secs(30)))
            // .set_provider_publication_interval(Some(Duration::from_secs(30)))
            .to_owned();

        let mut kademlia = Kademlia::with_config(
            local_peer_id,
            MemoryStore::new(local_peer_id),
            kademlia_config,
        );

        for known_peer in known_peers {
            info!(
                "Kademlia:  ---- adding peer:{} at {}",
                &known_peer.0, &known_peer.1
            );
            kademlia.add_address(&known_peer.0, known_peer.1.clone());
        }

        if let Err(store_error) = kademlia.start_providing("topos-tce".as_bytes().to_vec().into()) {
            warn!(reason = %store_error, "Could not start providing Kademlia protocol `topos-tce`")
        }

        if kademlia.bootstrap().is_err() {
            warn!("Bootstrapping failed because of NoKnownPeers, ignore this warning if boot-node");
        }

        kademlia
    }
}
