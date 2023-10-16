use self::{discovery::DiscoveryBehaviour, peer_info::PeerInfoBehaviour};
use crate::event::ComposedEvent;
use libp2p::swarm::NetworkBehaviour;

pub(crate) mod discovery;
pub(crate) mod gossip;
pub(crate) mod grpc;
pub(crate) mod peer_info;
pub(crate) mod topos;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ComposedEvent")]
pub(crate) struct Behaviour {
    /// Periodically pings and identifies the nodes we are connected to,
    /// and store information in a cache.
    pub(crate) peer_info: PeerInfoBehaviour,

    /// DiscoveryBehaviour which handle every aspect of the node discovery
    pub(crate) discovery: DiscoveryBehaviour,

    /// Gossip behaviour which handle the gossipsub protocol
    pub(crate) gossipsub: gossip::Behaviour,

    /// Custom gRPC behaviour which handle the different TOPOS gRPC protocol
    pub(crate) grpc: grpc::Behaviour,
}
