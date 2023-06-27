use std::env;

use lazy_static::lazy_static;

// TODO: Investigate BUFFER SIZE

lazy_static! {
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
}

pub const COMMAND_STREAM_BUFFER: usize = 2048;
pub const TRANSMISSION_PROTOCOL: &str = "/tce-transmission/1";
pub const DISCOVERY_PROTOCOL: &str = "/tce-disco/1";
