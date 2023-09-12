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
    EpochChange,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to generate spawn date")]
    SpawnDateFailure,
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use crate::{Clock, Event, TimeClock};

    #[tokio::test]
    async fn test_time_clock() {
        let genesis = Utc::now()
            .checked_sub_signed(Duration::seconds(30))
            .unwrap();

        let clock = TimeClock::new(genesis, 5).unwrap();
        let current_block = clock.block_ref();
        let current_epoch = clock.epoch_ref();

        let mut recv = clock.spawn().unwrap();

        assert_eq!(recv.recv().await, Ok(Event::EpochChange));
        assert_eq!(current_epoch.load(std::sync::atomic::Ordering::Relaxed), 7);
        assert_eq!(current_block.load(std::sync::atomic::Ordering::Relaxed), 30);
    }
}
