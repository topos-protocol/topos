//! Kademlia & Identify working together for auto discovery of the nodes
//!
use std::collections::HashSet;
use std::time::Duration;

use libp2p::{
    identify::Identify,
    identify::{IdentifyConfig, IdentifyEvent},
    kad::store::MemoryStore,
    kad::Kademlia,
    kad::{KademliaConfig, KademliaEvent},
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use tokio::sync::mpsc;

use crate::{identity::Keypair, NetworkEvents};

/// Auto discovery behaviour.
///
/// Based on Kademlia and Identify.
///
/// Note on *Identify* - default implementation takes 'listen_address' from the swarm.
///     However when working e.g. in docker externally visible ip-address will be different,
///     lets think about it on later stage.
///
#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
pub(crate) struct DiscoveryBehavior {
    kademlia: Kademlia<MemoryStore>,
    identify: Identify,

    #[behaviour(ignore)]
    tx_events: mpsc::UnboundedSender<NetworkEvents>,
    #[behaviour(ignore)]
    routable_peers: HashSet<PeerId>,
}

const TCE_TRANSMISSION_PROTOCOL: &str = "/trbp-transmission/1";
const TCE_DISCOVERY_PROTOCOL: &str = "/tce-disco/1";

impl DiscoveryBehavior {
    pub(crate) fn new(
        local_key: Keypair,
        known_peers: Vec<(PeerId, Multiaddr)>,
        tx_events: mpsc::UnboundedSender<NetworkEvents>,
    ) -> Self {
        let local_peer_id = local_key.public().to_peer_id();

        // identify
        let ident_config =
            IdentifyConfig::new(TCE_TRANSMISSION_PROTOCOL.to_string(), local_key.public())
                .with_push_listen_addr_updates(true);
        let ident = Identify::new(ident_config);

        // kademlia
        let kad_config = KademliaConfig::default()
            .set_protocol_name(TCE_DISCOVERY_PROTOCOL.as_bytes())
            .set_replication_interval(Some(Duration::from_secs(30)))
            .set_publication_interval(Some(Duration::from_secs(30)))
            .set_provider_publication_interval(Some(Duration::from_secs(30)))
            .to_owned();

        let mut kad =
            Kademlia::with_config(local_peer_id, MemoryStore::new(local_peer_id), kad_config);

        for known_peer in known_peers {
            log::info!(
                "Kademlia:  ---- adding peer:{} at {}",
                &known_peer.0,
                &known_peer.1
            );
            kad.add_address(&known_peer.0, known_peer.1);
        }

        match kad.bootstrap() {
            Err(_) => log::warn!("Kademlia: No peers found"),
            Ok(_) => log::info!("Kademlia: Started"),
        }

        kad.start_providing("topos-tce".as_bytes().to_vec().into())
            .expect("Registered at KadDHT");

        Self {
            kademlia: kad,
            identify: ident,
            tx_events,
            routable_peers: HashSet::new(),
        }
    }

    fn notify_peers(&mut self) {
        log::debug!("notify_peers - peers: {:?}", &self.routable_peers);
        let cl_tx = self.tx_events.clone();
        let cl_peers: Vec<PeerId> = Vec::from_iter(self.routable_peers.clone());
        tokio::spawn(async move {
            let _ = cl_tx.send(NetworkEvents::KadPeersChanged {
                new_peers: cl_peers,
            });
        });
    }
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for DiscoveryBehavior {
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Received { peer_id, info } => {
                log::debug!("IdentifyEvent: Received {{ {:?}, {:?} }}", &peer_id, &info);
                for ref la in info.listen_addrs {
                    self.kademlia.add_address(&peer_id, la.clone());
                }
            }
            _ => {
                log::debug!("IdentifyEvent (no process): {:?}", &event);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for DiscoveryBehavior {
    fn inject_event(&mut self, event: KademliaEvent) {
        log::debug!("KademliaEvent: {:?}", &event);
        match event {
            KademliaEvent::RoutingUpdated {
                peer,
                is_new_peer,
                addresses: _,
                bucket_range: _,
                old_peer: _,
            } => {
                log::info!("routing updated: {:?}", peer);
                // do the callback AFTER Identify worked (not a newly added peer)
                if !is_new_peer && self.routable_peers.insert(peer) {
                    self.notify_peers();
                }
            }
            KademliaEvent::UnroutablePeer { peer } => {
                log::info!("unroutable peer: {:?}", peer);
                if self.routable_peers.remove(&peer) {
                    self.notify_peers();
                }
            }
            _ => {}
        }
    }
}
