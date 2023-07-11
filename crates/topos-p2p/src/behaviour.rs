use self::{
    discovery::DiscoveryBehaviour, peer_info::PeerInfoBehaviour,
    transmission::codec::TransmissionCodec,
};
use crate::event::ComposedEvent;
use libp2p::{
    gossipsub,
    kad::{store::MemoryStore, Kademlia},
    request_response,
    swarm::keep_alive,
    swarm::NetworkBehaviour,
};

pub(crate) mod discovery;
pub(crate) mod gossip;
pub(crate) mod peer_info;
pub(crate) mod topos;
pub(crate) mod transmission;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ComposedEvent")]
pub(crate) struct Behaviour {
    /// All the topos-specific protocols.
    // pub(crate) topos: ToposBehaviour,

    /// Periodically pings and identifies the nodes we are connected to, and store information in a
    /// cache.
    pub(crate) peer_info: PeerInfoBehaviour,

    /// DiscoveryBehaviour which handle every aspect of the node discovery
    pub(crate) discovery: DiscoveryBehaviour,

    /// TransmissionBehaviour handle how we communicate with nodes
    pub(crate) transmission: request_response::Behaviour<TransmissionCodec>,

    pub(crate) keep_alive: keep_alive::Behaviour,

    pub(crate) gossipsub: gossip::Behaviour,
}
