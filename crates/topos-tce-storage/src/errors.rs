use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use topos_core::{
    types::stream::PositionError,
    uci::{CertificateId, SubnetId, SUBNET_ID_LENGTH},
};

#[derive(Error, Debug)]
pub enum InternalStorageError {
    #[error("The certificate already exists")]
    CertificateAlreadyExists,

    #[error("Unable to find a certificate: {0:?}")]
    CertificateNotFound(CertificateId),

    #[error("Unable to start storage")]
    UnableToStartStorage,

    #[cfg(feature = "rocksdb")]
    #[error("Unable to execute query: {0}")]
    RocksDBError(#[from] rocksdb::Error),

    #[cfg(feature = "rocksdb")]
    #[error("Accessing invalid column family: {0}")]
    InvalidColumnFamily(&'static str),

    #[error("Unable to deserialize database value")]
    UnableToDeserializeValue,

    #[error("Invalid query argument: {0}")]
    InvalidQueryArgument(&'static str),

    #[error(transparent)]
    Bincode(#[from] Box<bincode::ErrorKind>),

    #[error("A concurrent DBBatch has been detected")]
    ConcurrentDBBatchDetected,

    #[error("{0}: {1:?}")]
    PositionError(#[source] PositionError, [u8; SUBNET_ID_LENGTH]),

    #[error("InvalidSubnetId")]
    InvalidSubnetId,

    #[error("Missing head certificate for source subnet id {0}")]
    MissingHeadForSubnet(SubnetId),

    #[error("Certificate already exists at position {0} for subnet {1}")]
    CertificateAlreadyExistsAtPosition(u64, SubnetId),
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    InternalStorage(#[from] InternalStorageError),

    #[error("Unable to communicate with storage: closed")]
    CommunicationChannelClosed,

    #[error("Unable to receive expected response from storage: {0}")]
    ResponseChannel(#[from] oneshot::error::RecvError),

    #[error("Unable to execute shutdown on the storage service: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),
}
