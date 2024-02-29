use crate::runtime::{error::RuntimeError, InternalRuntimeCommand};
use thiserror::Error;
use tokio::sync::{mpsc::error::SendError, oneshot::error::RecvError};
use tonic::Code;
use uuid::Uuid;

#[derive(Error, Debug)]
pub(crate) enum StreamErrorKind {
    #[error(transparent)]
    HandshakeFailed(#[from] HandshakeError),
    #[error("Pre-start error")]
    PreStartError,
    #[error("Stream is closed")]
    StreamClosed,
    #[error("A timeout occurred")]
    Timeout,
    #[error("The submitted command is invalid")]
    InvalidCommand,
    #[error("Transport error: {0}")]
    Transport(Code),
    #[error("The submitted TargetCheckpoint is ill-formed")]
    MalformedTargetCheckpoint,
}

#[derive(Debug)]
pub struct StreamError {
    pub(crate) stream_id: Uuid,
    pub(crate) kind: StreamErrorKind,
}

impl StreamError {
    pub(crate) fn new(stream_id: Uuid, kind: StreamErrorKind) -> Self {
        Self { stream_id, kind }
    }
}

#[derive(Error, Debug)]
pub(crate) enum HandshakeError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    OneshotCommunicationChannel(#[from] RecvError),

    #[error(transparent)]
    InternalCommunicationChannel(#[from] Box<SendError<InternalRuntimeCommand>>),
}

#[cfg(test)]
mod tests {
    use test_log::test;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    use super::*;

    #[test(tokio::test)]
    async fn handshake_error_expected() {
        let uuid = Uuid::new_v4();
        let runtime_error = RuntimeError::PendingStreamNotFound(uuid);

        let handshake_error: HandshakeError = runtime_error.into();

        assert_eq!(
            format!("The pending stream {uuid} was not found"),
            handshake_error.to_string()
        );

        let (sender, receiver) = oneshot::channel::<Result<(), RuntimeError>>();

        drop(sender);

        let handshake_error: HandshakeError = receiver.await.unwrap_err().into();
        assert_eq!("channel closed", handshake_error.to_string());
    }
}
