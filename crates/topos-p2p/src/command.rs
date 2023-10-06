use std::fmt::Display;

use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{
    behaviour::transmission::codec::TransmissionResponse,
    error::{CommandExecutionError, P2PError},
};

#[derive(Debug)]
pub enum Command {
    /// Executed when the node is starting
    StartListening {
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), P2PError>>,
    },

    /// Command to initiate a dial with another peer.
    /// If we already dialled the peer, an error is returned
    /// If the peer that we want to dial is self, an error is returned
    /// If we can't initiate a dial with the peer, an error is returned
    Dial {
        peer_id: PeerId,
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), P2PError>>,
    },

    /// Command to ask for the current connected peer id list
    ConnectedPeers {
        sender: oneshot::Sender<Result<Vec<PeerId>, P2PError>>,
    },

    /// Disconnect the node
    Disconnect {
        sender: oneshot::Sender<Result<(), P2PError>>,
    },

    /// Send a TransmissionReq to multiple nodes
    TransmissionReq {
        to: PeerId,
        data: Vec<u8>,
        protocol: &'static str,
        sender: oneshot::Sender<Result<Vec<u8>, CommandExecutionError>>,
    },

    /// Try to discover a peer based on its PeerId
    Discover {
        to: PeerId,
        sender: oneshot::Sender<Result<Vec<Multiaddr>, CommandExecutionError>>,
    },

    /// Send a TransmissionReq to multiple nodes
    TransmissionResponse {
        data: Result<Vec<u8>, ()>,
        protocol: &'static str,
        channel: ResponseChannel<Result<TransmissionResponse, ()>>,
    },

    Gossip {
        topic: &'static str,
        data: Vec<u8>,
    },
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::StartListening { .. } => write!(f, "StartListening"),
            Command::Dial { peer_id, .. } => write!(f, "Dial({peer_id})"),
            Command::ConnectedPeers { .. } => write!(f, "ConnectedPeers"),
            Command::Disconnect { .. } => write!(f, "Disconnect"),
            Command::TransmissionReq { to, .. } => write!(f, "TransmissionReq(to: {to})"),
            Command::Gossip { .. } => write!(f, "GossipMessage"),
            Command::Discover { to, .. } => write!(f, "Discover(to: {to})"),
            Command::TransmissionResponse { .. } => write!(f, "TransmissionResponse"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotReadyMessage {}
