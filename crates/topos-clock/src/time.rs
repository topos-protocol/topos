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
use tracing::info;

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

            self.current_block.fetch_add(1, Ordering::Relaxed);
            info!("current block: {:?}", self.current_block);

            if self.current_block.load(Ordering::Relaxed) % self.epoch_duration == 0 {
                self.compute_epoch();
                _ = sender.send(Event::EpochChange);
            }
        }
    }

    fn compute_block(&mut self) {
        let current = Utc::now();
        let x = current
            .naive_utc()
            .signed_duration_since(self.genesis.naive_utc());

        let blocks = x.num_seconds();

        self.current_block = Arc::new(if blocks.is_negative() {
            AtomicU64::new(0)
        } else {
            AtomicU64::new(blocks as u64)
        });
    }

    fn compute_epoch(&mut self) {
        self.current_epoch.store(
            self.current_block.load(Ordering::Relaxed) / self.epoch_duration,
            Ordering::Relaxed,
        );
    }
}
