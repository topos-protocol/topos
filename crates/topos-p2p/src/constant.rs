use std::env;

use lazy_static::lazy_static;
use prometheus_client::registry::Registry;
use tokio::sync::Mutex;

lazy_static! {
    /// Metric Registry used to register all the metrics from libp2p::gossipsub
    pub static ref METRIC_REGISTRY: Mutex<Registry> = Mutex::new(<Registry>::with_prefix("topos"));
    pub static ref EVENT_STREAM_BUFFER: usize = env::var("TCE_EVENT_STREAM_BUFFER")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(2048 * 2);
    pub static ref CAPACITY_EVENT_STREAM_BUFFER: usize = EVENT_STREAM_BUFFER
        .checked_mul(10)
        .map(|v| {
            let r: usize = v.checked_div(100).unwrap_or(*EVENT_STREAM_BUFFER);
            r
        })
        .unwrap_or(*EVENT_STREAM_BUFFER);
    pub static ref COMMAND_STREAM_BUFFER_SIZE: usize = env::var("TCE_COMMAND_STREAM_BUFFER_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(2048);
}

pub const TRANSMISSION_PROTOCOL: &str = "/tce-transmission/1";
pub const DISCOVERY_PROTOCOL: &str = "/tce-disco/1";
pub const PEER_INFO_PROTOCOL: &str = "/tce-peer-info/1";
