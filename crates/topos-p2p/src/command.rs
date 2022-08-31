use std::error::Error;

use libp2p::{request_response::ResponseChannel, PeerId};
use tokio::sync::oneshot;

use crate::behaviour::transmission::codec::TransmissionResponse;

#[derive(Debug)]
pub enum Command {
    /// Executed when the node is starting
    StartListening {
        peer_addr: topos_addr::ToposAddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },

    /// Command to initiate a dial with another peer.
    /// If we already dialled the peer, an error is returned
    /// If the peer that we want to dial is self, an error is returned
    /// If we can't initiate a dial with the peer, an error is returned
    Dial {
        peer_id: PeerId,
        peer_addr: topos_addr::ToposAddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
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

    /// Send a TransmissionReq to multiple nodes
    TransmissionResponse {
        data: Vec<u8>,
        channel: ResponseChannel<TransmissionResponse>,
    },
}
