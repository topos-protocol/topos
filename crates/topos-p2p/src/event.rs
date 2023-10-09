use libp2p::{
    gossipsub::{self, Event as GossipsubEvent},
    identify,
    kad::KademliaEvent,
    request_response::{Event as RequestResponseEvent, ResponseChannel},
    PeerId,
};

use crate::behaviour::{
    grpc,
    transmission::codec::{TransmissionRequest, TransmissionResponse},
};

#[derive(Debug)]
pub struct GossipEvent {
    pub source: Option<PeerId>,
    pub topic: &'static str,
    pub message: Vec<u8>,
}

#[derive(Debug)]
pub enum ComposedEvent {
    Kademlia(Box<KademliaEvent>),
    Transmission(RequestResponseEvent<TransmissionRequest, Result<TransmissionResponse, ()>>),
    PeerInfo(Box<identify::Event>),
    Gossipsub(GossipEvent),
    Grpc(grpc::Event),
    Void,
}
impl From<grpc::Event> for ComposedEvent {
    fn from(event: grpc::Event) -> Self {
        ComposedEvent::Grpc(event)
    }
}

impl From<KademliaEvent> for ComposedEvent {
    fn from(event: KademliaEvent) -> Self {
        ComposedEvent::Kademlia(Box::new(event))
    }
}

impl From<identify::Event> for ComposedEvent {
    fn from(event: identify::Event) -> Self {
        ComposedEvent::PeerInfo(Box::new(event))
    }
}

impl From<RequestResponseEvent<TransmissionRequest, Result<TransmissionResponse, ()>>>
    for ComposedEvent
{
    fn from(
        event: RequestResponseEvent<TransmissionRequest, Result<TransmissionResponse, ()>>,
    ) -> Self {
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
    SynchronizerRequest {
        from: PeerId,
        data: Vec<u8>,
        channel: ResponseChannel<Result<TransmissionResponse, ()>>,
    },
    TransmissionOnReq {
        from: PeerId,
        data: Vec<u8>,
        channel: ResponseChannel<Result<TransmissionResponse, ()>>,
    },
}
