use crate::runtime::{error::RuntimeError, InternalRuntimeCommand};
use thiserror::Error;
use tokio::sync::{mpsc::error::SendError, oneshot::error::RecvError};

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
