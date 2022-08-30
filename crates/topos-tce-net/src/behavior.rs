//! Our network behavior.
//!
//! Serves the purposes of:
//! - nodes discovery and advertising
//! - sampling peers per protocol definition
//! - exchanging protocol related data
//!

use crate::discovery_behavior::DiscoveryBehavior;
use crate::{transmission_behavior::TransmissionBehavior, NetworkEvents};
use libp2p::{identity::Keypair, Multiaddr, NetworkBehaviour, PeerId};
use tokio::sync::mpsc;

/// Our combined network behavior.
#[derive(NetworkBehaviour)]
pub(crate) struct Behavior {
    /// Messages transmission
    pub transmission: TransmissionBehavior,

    /// Discovery
    pub discovery: DiscoveryBehavior,
}

impl Behavior {
    pub fn new(
        local_key: Keypair,
        known_peers: Vec<(PeerId, Multiaddr)>,
        tx_events: mpsc::UnboundedSender<NetworkEvents>,
    ) -> Self {
        Self {
            transmission: TransmissionBehavior::new(tx_events.clone()),
            discovery: DiscoveryBehavior::new(local_key, known_peers, tx_events),
        }
    }
}
