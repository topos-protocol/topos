use std::{error::Error, fmt::Display};

use libp2p::{request_response::ResponseChannel, Multiaddr, PeerId};
use tokio::sync::oneshot;

use crate::{behaviour::transmission::codec::TransmissionResponse, error::FSMError};

#[derive(Debug)]
pub enum Command {
    /// Executed when the node is starting
    StartListening {
        peer_addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
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
        sender: oneshot::Sender<Result<Vec<PeerId>, Box<dyn Error + Send>>>,
    },

    /// Disconnect the node
    Disconnect {
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },

    /// Send a TransmissionReq to multiple nodes
    TransmissionReq {
        to: PeerId,
        data: Vec<u8>,
        sender: oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
    },

    /// Try to discover a peer based on its PeerId
    Discover {
        to: PeerId,
        sender: oneshot::Sender<Result<Vec<Multiaddr>, Box<dyn Error + Send>>>,
    },

    /// Send a TransmissionReq to multiple nodes
    TransmissionResponse {
        data: Vec<u8>,
        channel: ResponseChannel<TransmissionResponse>,
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
            Command::Discover { to, .. } => write!(f, "Discover(to: {to})"),
            Command::TransmissionResponse { .. } => write!(f, "TransmissionResponse"),
        }
    }
}
