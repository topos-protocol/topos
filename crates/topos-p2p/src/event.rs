use libp2p::{identify, kad, PeerId};

use crate::behaviour::grpc;

#[derive(Debug)]
pub struct GossipEvent {
    pub source: Option<PeerId>,
    pub topic: &'static str,
    pub message: Vec<u8>,
}

#[derive(Debug)]
pub enum ComposedEvent {
    Kademlia(Box<kad::Event>),
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

impl From<kad::Event> for ComposedEvent {
    fn from(event: kad::Event) -> Self {
        ComposedEvent::Kademlia(Box::new(event))
    }
}

impl From<identify::Event> for ComposedEvent {
    fn from(event: identify::Event) -> Self {
        ComposedEvent::PeerInfo(Box::new(event))
    }
}

impl From<void::Void> for ComposedEvent {
    fn from(_: void::Void) -> Self {
        Self::Void
    }
}

#[derive(Debug)]
pub enum Event {
    Gossip { from: PeerId, data: Vec<u8> },
}
