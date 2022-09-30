use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use topos_core::uci::CertificateId;

use crate::command::StorageCommand;

#[derive(Error, Debug)]
pub enum InternalStorageError {
    #[error("The certificate already exists")]
    CertificateAlreadyExists,
    #[error("Unable to find a certificate: {0}")]
    CertificateNotFound(CertificateId),
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    InternalStorage(#[from] InternalStorageError),

    #[error("Unable to communicate with storage: {0}")]
    CommunicationChannel(#[from] mpsc::error::SendError<StorageCommand>),

    #[error("Unable to communicate with storage: closed")]
    CommunicationChannelClosed,

    #[error("Unable to receive expected response from storage: {0}")]
    ResponseChannel(#[from] oneshot::error::RecvError),
}
