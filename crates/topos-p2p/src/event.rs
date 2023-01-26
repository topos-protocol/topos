use libp2p::{
    identify,
    kad::KademliaEvent,
    request_response::{RequestResponseEvent, ResponseChannel},
    PeerId,
};

use crate::behaviour::transmission::codec::{TransmissionRequest, TransmissionResponse};

#[derive(Debug)]
pub enum ComposedEvent {
    Kademlia(KademliaEvent),
    Transmission(RequestResponseEvent<TransmissionRequest, TransmissionResponse>),
    #[allow(dead_code)]
    OutEvent(Event),
    PeerInfo(Box<identify::Event>),
    Void,
}

impl From<KademliaEvent> for ComposedEvent {
    fn from(event: KademliaEvent) -> Self {
        ComposedEvent::Kademlia(event)
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
    TransmissionOnReq {
        from: PeerId,
        data: Vec<u8>,
        channel: ResponseChannel<TransmissionResponse>,
    },
}
