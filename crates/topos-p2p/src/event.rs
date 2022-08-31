use libp2p::{
    kad::KademliaEvent,
    request_response::{RequestResponseEvent, ResponseChannel},
    PeerId,
};

use crate::behaviour::transmission::codec::{TransmissionRequest, TransmissionResponse};

#[derive(Debug)]
pub enum ComposedEvent {
    Kademlia(KademliaEvent),
    Transmission(RequestResponseEvent<TransmissionRequest, TransmissionResponse>),
    OutEvent(Event),
}

impl From<KademliaEvent> for ComposedEvent {
    fn from(event: KademliaEvent) -> Self {
        ComposedEvent::Kademlia(event)
    }
}

impl From<RequestResponseEvent<TransmissionRequest, TransmissionResponse>> for ComposedEvent {
    fn from(event: RequestResponseEvent<TransmissionRequest, TransmissionResponse>) -> Self {
        Self::Transmission(event)
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
    TransmissionOnReq {
        from: PeerId,
        data: Vec<u8>,
        channel: ResponseChannel<TransmissionResponse>,
    },
}
