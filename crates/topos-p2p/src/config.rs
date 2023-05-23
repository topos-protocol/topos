use std::{num::NonZeroUsize, time::Duration};

pub struct NetworkConfig {
    pub publish_retry: usize,
    pub minimum_cluster_size: usize,
    pub client_retry_ttl: u64,
    pub discovery: DiscoveryConfig,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            publish_retry: 10,
            minimum_cluster_size: 5,
            client_retry_ttl: 2000,
            discovery: Default::default(),
        }
    }
}

pub struct DiscoveryConfig {
    pub replication_factor: NonZeroUsize,
    pub replication_interval: Option<Duration>,
    pub publication_interval: Option<Duration>,
    pub provider_publication_interval: Option<Duration>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            replication_factor: NonZeroUsize::new(4).unwrap(),
            replication_interval: Some(Duration::from_secs(10)),
            publication_interval: Some(Duration::from_secs(10)),
            provider_publication_interval: Some(Duration::from_secs(10)),
        }
    }
}
