use self::{
    discovery::{DiscoveryBehaviour, DiscoveryOut},
    peer_info::{PeerInfoBehaviour, PeerInfoOut},
    transmission::{protocol::TransmissionProtocol, TransmissionBehaviour, TransmissionOut},
};
use crate::{event::ComposedEvent, Event};
use libp2p::swarm::{
    NetworkBehaviour, NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters,
};
use libp2p::{core::ProtocolName, identify::IdentifyInfo, NetworkBehaviour};
use std::{
    collections::VecDeque,
    task::{Context, Poll},
};

pub(crate) mod discovery;
pub(crate) mod peer_info;
pub(crate) mod topos;
pub(crate) mod transmission;

#[derive(NetworkBehaviour)]
#[behaviour(
    event_process = true,
    poll_method = "poll",
    out_event = "ComposedEvent"
)]
pub(crate) struct Behaviour {
    /// All the topos-specific protocols.
    // pub(crate) topos: ToposBehaviour,

    /// Periodically pings and identifies the nodes we are connected to, and store information in a
    /// cache.
    pub(crate) peer_info: PeerInfoBehaviour,

    /// DiscoveryBehaviour which handle every aspect of the node discovery
    pub(crate) discovery: DiscoveryBehaviour,

    // pub(crate) transmission: RequestResponse<TransmissionCodec>,
    /// TransmissionBehaviour handle how we communicate with nodes
    pub(crate) transmission: TransmissionBehaviour,

    /// Buffer of events to send to the Runtime
    #[behaviour(ignore)]
    pub(crate) events: VecDeque<ComposedEvent>,
}

impl Behaviour {
    fn poll(
        &mut self,
        _cx: &mut Context,
        _: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<ComposedEvent, <Self as NetworkBehaviour>::ConnectionHandler>>
    {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(event));
        }

        Poll::Pending
    }
}

impl NetworkBehaviourEventProcess<TransmissionOut> for Behaviour {
    fn inject_event(&mut self, event: TransmissionOut) {
        match event {
            TransmissionOut::Request {
                from,
                data,
                channel,
            } => self
                .events
                .push_back(ComposedEvent::OutEvent(Event::TransmissionOnReq {
                    from,
                    data,
                    channel,
                })),
        }
    }
}

impl NetworkBehaviourEventProcess<DiscoveryOut> for Behaviour {
    fn inject_event(&mut self, event: DiscoveryOut) {
        match event {
            DiscoveryOut::Discovered(peer) => {
                if self.discovery.peers.insert(peer) {
                    self.events
                        .push_back(ComposedEvent::OutEvent(Event::PeersChanged {
                            new_peers: vec![peer],
                        }))
                }
            }
            DiscoveryOut::UnroutablePeer(peer) => {
                if self.discovery.peers.remove(&peer) {
                    let peers = self.discovery.peers.iter().cloned().collect();
                    self.events
                        .push_back(ComposedEvent::OutEvent(Event::PeersChanged {
                            new_peers: peers,
                        }))
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<PeerInfoOut> for Behaviour {
    fn inject_event(&mut self, event: PeerInfoOut) {
        match event {
            PeerInfoOut::Identified { peer_id, info } => {
                let IdentifyInfo {
                    protocol_version,
                    listen_addrs,
                    protocols,
                    ..
                } = *info;

                if protocol_version.as_bytes() == TransmissionProtocol().protocol_name()
                    && protocols
                        .iter()
                        .any(|p| p.as_bytes() == self.discovery.kademlia.protocol_name())
                {
                    for addr in listen_addrs {
                        self.transmission.inner.add_address(&peer_id, addr.clone());
                        self.discovery.kademlia.add_address(&peer_id, addr);
                    }
                }
            }

            PeerInfoOut::Disconnected { peer_id } => {
                if self.discovery.peers.remove(&peer_id) {
                    self.events
                        .push_back(ComposedEvent::OutEvent(Event::PeerDisconnected { peer_id }));

                    let peers = self.discovery.peers.iter().cloned().collect();
                    self.events
                        .push_back(ComposedEvent::OutEvent(Event::PeersChanged {
                            new_peers: peers,
                        }))
                }
            }
        }
    }
}
