use proto::instrument::{network_update::Update, ConnectedPeer, DisconnectedPeer};
use tracing::field::Visit;

use topos_tce_console_api as proto;

#[derive(Debug)]
pub enum NetworkEvent {
    PeerConnection {
        peer_id: String,
        direction: String,
        addresses: String,
    },

    PeerDisconnection {
        peer_id: String,
        direction: String,
        addresses: String,
    },
}

impl NetworkEvent {
    pub fn to_proto(&self) -> proto::instrument::NetworkUpdate {
        proto::instrument::NetworkUpdate {
            update: match self {
                NetworkEvent::PeerConnection {
                    peer_id,
                    direction,
                    addresses,
                } => Some(Update::ConnectedPeer(ConnectedPeer {
                    peer_id: peer_id.clone(),
                    addresses: addresses.clone(),
                    direction: direction.clone(),
                })),
                NetworkEvent::PeerDisconnection { peer_id, .. } => {
                    Some(Update::DisconnectedPeer(DisconnectedPeer {
                        peer_id: peer_id.clone(),
                    }))
                }
            },
        }
    }
}
#[derive(PartialEq, Debug)]
pub enum NetworkEventType {
    ConnectionClosed,
    ConnectionEstablished,
}

#[derive(Default, Debug)]
pub struct NetworkVisitor {
    event: Option<NetworkEventType>,
    peer_id: Option<String>,
    direction: Option<String>,
    addresses: Option<String>,
}

impl NetworkVisitor {
    fn check_message(&mut self, message: String) {
        match message.to_lowercase() {
            m if m.starts_with("connection closed") => {
                self.event = Some(NetworkEventType::ConnectionClosed)
            }
            m if m.starts_with("connection established") => {
                self.event = Some(NetworkEventType::ConnectionEstablished)
            }
            _ => {}
        }
    }

    pub(crate) fn result(self) -> Option<NetworkEvent> {
        match self.event {
            Some(event_type) => match event_type {
                NetworkEventType::ConnectionEstablished | NetworkEventType::ConnectionClosed
                    if self.peer_id.is_some()
                        && self.direction.is_some()
                        && self.addresses.is_some() =>
                {
                    if event_type == NetworkEventType::ConnectionEstablished {
                        Some(NetworkEvent::PeerConnection {
                            peer_id: self.peer_id.unwrap(),
                            direction: self.direction.unwrap(),
                            addresses: self.addresses.unwrap(),
                        })
                    } else {
                        Some(NetworkEvent::PeerDisconnection {
                            peer_id: self.peer_id.unwrap(),
                            direction: self.direction.unwrap(),
                            addresses: self.addresses.unwrap(),
                        })
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }
}

impl Visit for NetworkVisitor {
    fn record_debug(&mut self, field: &tracing_core::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.check_message(format!("{:?}", value)),
            _ => {}
        }
    }

    fn record_str(&mut self, field: &tracing_core::Field, value: &str) {
        match field.name() {
            "peer_id" => self.peer_id = Some(value.into()),
            "direction" => self.direction = Some(value.into()),
            "addresses" => self.addresses = Some(value.into()),
            _ => {}
        }
    }
}
