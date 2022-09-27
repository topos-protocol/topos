use libp2p::PeerId;
use thiserror::Error;

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
}
