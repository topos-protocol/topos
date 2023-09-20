//! This crate is responsible for managing the clock pace.
//!
//! The Clock is responsible of giving informations about Epoch and Delta timing by exposing
//! reference to the data but also by broadcasting `EpochChange` events.

use std::sync::{atomic::AtomicU64, Arc};

use tokio::sync::broadcast;

mod time;

pub use time::TimeClock;

const BROADCAST_CHANNEL_SIZE: usize = 100;

pub trait Clock {
    /// Compute Epoch/Block numbers and spawn the clock task.
    fn spawn(self) -> Result<broadcast::Receiver<Event>, Error>;
    /// Return a reference to the current block number
    fn block_ref(&self) -> Arc<AtomicU64>;
    /// Return a reference to the current epoch number
    fn epoch_ref(&self) -> Arc<AtomicU64>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Notify an Epoch change with the associated epoch_number
    EpochChange(u64),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to generate spawn date")]
    SpawnDateFailure,
}
