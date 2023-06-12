use std::io;

use libp2p::{
    noise::Error as NoiseError, request_response::OutboundFailure, PeerId, TransportError,
};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::command::Command;

#[derive(Error, Debug)]
pub enum P2PError {
    #[error("Can't dial on self")]
    CantDialSelf,
    #[error("Already dialed {0}")]
    AlreadyDialed(PeerId),
    #[error("Already disconnected")]
    AlreadyDisconnected,
    #[error("Error during dialling")]
    DialError,
    #[error("Unable build a network: peer_key missing")]
    MissingPeerKey,

    #[error(transparent)]
    CommandError(#[from] CommandExecutionError),

    #[error("An error occured on the Transport layer: {0}")]
    TransportError(#[from] TransportError<io::Error>),

    #[error("Unable to receive expected response of a oneshot channel")]
    OneshotReceiveError(#[from] oneshot::error::RecvError),

    #[error("An error occurred on the Noise protocol: {0}")]
    NoiseProtocolError(#[from] NoiseError),

    #[error("Error during bootstrap phase: {0}")]
    BootstrapError(&'static str),

    #[error("Unable to execute shutdown on the p2p runtime: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),
}

#[derive(Error, Debug)]
pub enum CommandExecutionError {
    #[error("Unable to send command {0}")]
    UnableToSendCommand(Command),

    #[error("Unable to perform query: {0}")]
    RequestOutbandFailure(#[from] OutboundFailure),

    #[error("Unable to receive expected response of a oneshot channel")]
    UnableToReceiveCommandResponse(#[from] oneshot::error::RecvError),

    #[error("Unable to send a command: {0}")]
    SendError(#[from] mpsc::error::SendError<Command>),

    #[error("Failed to fetch Record from DHT")]
    DHTGetRecordFailed,

    #[error("Connection with a peer has failed")]
    ConnectionClosed,
}
