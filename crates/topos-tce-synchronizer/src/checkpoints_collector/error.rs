use thiserror::Error;
use tokio::sync::oneshot::error::RecvError;

#[derive(Error, Debug)]
pub enum CheckpointsCollectorError {
    #[error("Unable to start the CheckpointsCollector")]
    UnableToStart,

    #[error("Unable to start the CheckpointsCollector: No gatekeeper client provided")]
    NoGatekeeperClient,

    #[error("Unable to start the CheckpointsCollector: No network client provided")]
    NoNetworkClient,

    #[error("Error while dealing with Start command: already starting")]
    AlreadyStarting,

    #[error("Error while trying to fetch random peers")]
    UnableToFetchRandomPeer,

    #[error(transparent)]
    OneshotCommunicationChannel(#[from] RecvError),

    #[error("Unable to start the CheckpointsCollector: No store provided")]
    NoStore,
}
