use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("The pending stream {0} was not found")]
    PendingStreamNotFound(Uuid),
}
