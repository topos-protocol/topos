use libp2p::PeerId;
use thiserror::Error;

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
    #[error("Unable to send command {0}")]
    UnableToSendCommand(Command),
}
