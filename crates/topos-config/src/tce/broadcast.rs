use serde::{Deserialize, Serialize};

/// Broadcast threshold configurations
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ReliableBroadcastParams {
    /// Echo threshold
    pub echo_threshold: usize,
    /// Ready threshold
    pub ready_threshold: usize,
    /// Delivery threshold
    pub delivery_threshold: usize,
}

impl ReliableBroadcastParams {
    pub const fn new(n: usize) -> Self {
        let f: usize = n / 3;

        Self {
            echo_threshold: 1 + (n + f) / 2,
            ready_threshold: 1 + f,
            delivery_threshold: 2 * f + 1,
        }
    }
}
