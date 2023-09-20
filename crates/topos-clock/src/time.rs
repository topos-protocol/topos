use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use chrono::{DateTime, Utc};
use tokio::{
    spawn,
    sync::broadcast,
    time::{interval_at, Instant},
};

use crate::{Clock, Error, Event, BROADCAST_CHANNEL_SIZE};

/// Time based clock implementation.
///
/// Simulate blockchain block production by increasing block number by 1 every second.
/// Epoch duration can be configured when creating the clock.
pub struct TimeClock {
    genesis: DateTime<Utc>,
    current_block: Arc<AtomicU64>,
    epoch_duration: u64,
    current_epoch: Arc<AtomicU64>,
}

impl Clock for TimeClock {
    fn spawn(mut self) -> Result<broadcast::Receiver<Event>, Error> {
        let (sender, receiver) = broadcast::channel(BROADCAST_CHANNEL_SIZE);

        self.compute_block();
        self.compute_epoch();

        spawn(async move {
            self.run(sender).await;
        });

        Ok(receiver)
    }

    fn block_ref(&self) -> Arc<AtomicU64> {
        self.current_block.clone()
    }
    fn epoch_ref(&self) -> Arc<AtomicU64> {
        self.current_epoch.clone()
    }
}

impl TimeClock {
    /// Create a new TimeClock instance based on a genesis datatime and an epoch duration.
    pub fn new(genesis: DateTime<Utc>, epoch_duration: u64) -> Result<Self, Error> {
        let mut clock = Self {
            genesis,
            current_block: Arc::new(AtomicU64::new(0)),
            epoch_duration,
            current_epoch: Arc::new(AtomicU64::new(0)),
        };

        clock.compute_block();
        clock.compute_epoch();

        Ok(clock)
    }

    async fn run(&mut self, sender: broadcast::Sender<Event>) {
        let mut interval = interval_at(Instant::now(), Duration::from_secs(1));
        loop {
            interval.tick().await;

            let _previous_block = self.current_block.fetch_add(1, Ordering::Relaxed);

            if self.current_block.load(Ordering::Relaxed) % self.epoch_duration == 0 {
                self.compute_epoch();
                _ = sender.send(Event::EpochChange(
                    self.current_epoch.load(Ordering::Relaxed),
                ));
            }
        }
    }

    fn compute_block(&mut self) {
        let blocks = std::cmp::max(
            Utc::now()
                .naive_utc()
                .signed_duration_since(self.genesis.naive_utc())
                .num_seconds(),
            0,
        ) as u64;

        self.current_block.store(blocks, Ordering::Relaxed);
    }

    fn compute_epoch(&mut self) {
        self.current_epoch.store(
            self.current_block.load(Ordering::Relaxed) / self.epoch_duration,
            Ordering::Relaxed,
        );
    }
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

        assert_eq!(recv.recv().await, Ok(Event::EpochChange(7)));
        assert_eq!(current_epoch.load(std::sync::atomic::Ordering::Relaxed), 7);
        assert!(current_block.load(std::sync::atomic::Ordering::Relaxed) >= 30);
    }

    #[tokio::test]
    async fn test_time_clock_catchup() {
        let genesis = Utc::now()
            .checked_sub_signed(Duration::seconds(30))
            .unwrap();

        let clock = TimeClock::new(genesis, 2).unwrap();
        let current_block = clock.block_ref();
        let current_epoch = clock.epoch_ref();

        let mut recv = clock.spawn().unwrap();

        assert_eq!(recv.recv().await, Ok(Event::EpochChange(16)));
        assert!(recv.try_recv().is_err());
        assert_eq!(current_epoch.load(std::sync::atomic::Ordering::Relaxed), 16);
        assert!(current_block.load(std::sync::atomic::Ordering::Relaxed) >= 30);
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        assert_eq!(recv.recv().await, Ok(Event::EpochChange(17)));
        assert_eq!(recv.recv().await, Ok(Event::EpochChange(18)));

        assert_eq!(current_epoch.load(std::sync::atomic::Ordering::Relaxed), 18);
        assert!(current_block.load(std::sync::atomic::Ordering::Relaxed) >= 35);
    }
}
