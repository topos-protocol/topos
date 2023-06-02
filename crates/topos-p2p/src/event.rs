use libp2p::{
    gossipsub::{self, Event as GossipsubEvent},
    identify,
    kad::KademliaEvent,
    request_response::{Event as RequestResponseEvent, ResponseChannel},
    PeerId,
};

use crate::behaviour::transmission::codec::{TransmissionRequest, TransmissionResponse};

#[derive(Debug)]
pub enum ComposedEvent {
    Kademlia(Box<KademliaEvent>),
    Transmission(RequestResponseEvent<TransmissionRequest, TransmissionResponse>),
    #[allow(dead_code)]
    OutEvent(Event),
    PeerInfo(Box<identify::Event>),
    Gossipsub(Box<GossipsubEvent>),
    Void,
}

impl From<KademliaEvent> for ComposedEvent {
    fn from(event: KademliaEvent) -> Self {
        ComposedEvent::Kademlia(Box::new(event))
    }
}

impl From<GossipsubEvent> for ComposedEvent {
    fn from(event: GossipsubEvent) -> Self {
        ComposedEvent::Gossipsub(Box::new(event))
    }
}

impl From<identify::Event> for ComposedEvent {
    fn from(event: identify::Event) -> Self {
        ComposedEvent::PeerInfo(Box::new(event))
    }
}

impl From<RequestResponseEvent<TransmissionRequest, TransmissionResponse>> for ComposedEvent {
    fn from(event: RequestResponseEvent<TransmissionRequest, TransmissionResponse>) -> Self {
        Self::Transmission(event)
    }
}

impl From<void::Void> for ComposedEvent {
    fn from(_: void::Void) -> Self {
        Self::Void
    }
}

#[derive(Debug)]
pub enum Event {
    PeerDisconnected {
        peer_id: PeerId,
    },
    PeersChanged {
        new_peers: Vec<PeerId>,
    },
    Gossip {
        from: PeerId,
        data: Vec<u8>,
    },
    TransmissionOnReq {
        from: PeerId,
        data: Vec<u8>,
        channel: ResponseChannel<TransmissionResponse>,
    },
}
