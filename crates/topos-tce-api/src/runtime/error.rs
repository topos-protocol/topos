use thiserror::Error;
use topos_core::uci::SubnetId;
use topos_tce_storage::errors::StorageError;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("The pending stream {0} was not found")]
    PendingStreamNotFound(Uuid),

    #[error("Unable to get source head certificate for subnet id {0}: {1}")]
    UnableToGetSourceHead(SubnetId, String),

    #[error("Unknown subnet with subnet id {0}")]
    UnknownSubnet(SubnetId),

    #[error("Unexpected store error: {0}")]
    Store(#[from] StorageError),

    #[error("Communication error: {0}")]
    CommunicationError(String),
}
