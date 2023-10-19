use std::sync::Arc;

use tokio::sync::oneshot;

#[derive(Debug, thiserror::Error)]
pub enum OutboundError {
    #[error("Unable to Dial")]
    DialFailure,
    #[error("Peer doesn't support the protocol: {0}")]
    UnsupportedProtocol(String),
    #[error(transparent)]
    GrpcChannel(#[from] Arc<tonic::transport::Error>),
    #[error("Outbound connection timeout")]
    Timeout,
}

#[derive(thiserror::Error, Debug)]
pub enum OutboundConnectionError {
    #[error(transparent)]
    Outbound(#[from] OutboundError),
    #[error(transparent)]
    ConnectionCanceled(#[from] oneshot::error::RecvError),
    #[error("This connection is already negotiating with another client")]
    AlreadyNegotiating,
}
