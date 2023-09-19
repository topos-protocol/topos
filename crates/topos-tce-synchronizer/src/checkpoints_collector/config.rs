pub struct CheckpointsCollectorConfig {
    pub(crate) sync_interval_seconds: u64,
}

impl CheckpointsCollectorConfig {
    const SYNC_INTERVAL_SECONDS: u64 = 10;
}

impl Default for CheckpointsCollectorConfig {
    fn default() -> Self {
        Self {
            sync_interval_seconds: Self::SYNC_INTERVAL_SECONDS,
        }
    }
}
