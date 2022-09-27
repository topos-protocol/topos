use std::io;

use libp2p::{noise::NoiseError, request_response::OutboundFailure, PeerId, TransportError};
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
    #[error("Unable build a network because of the peer_key is missing")]
    MissingPeerKey,

    #[error(transparent)]
    CommandError(#[from] CommandExecutionError),

    #[error("An error occured on the Transport layer: {0}")]
    TransportError(#[from] TransportError<io::Error>),

    #[error("Unable to received expected response of a oneshot channel")]
    OneshotReceiveError(#[from] oneshot::error::RecvError),

    #[error("An error occured on the Noice protocol: {0}")]
    NoiseProtocolError(#[from] NoiseError),
}

#[derive(Error, Debug)]
pub enum CommandExecutionError {
    #[error("Unable to send command {0}")]
    UnableToSendCommand(Command),

    #[error("Unable to perform query: {0}")]
    RequestOutbandFailure(#[from] OutboundFailure),

    #[error("Unable to received expected response of a oneshot channel")]
    UnableToReceiveCommandResponse(#[from] oneshot::error::RecvError),

    #[error("Unable to send a command: {0}")]
    SendError(#[from] mpsc::error::SendError<Command>),
}
