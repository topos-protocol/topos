use lazy_static::lazy_static;

lazy_static! {
    pub static ref COMMAND_CHANNEL_SIZE: usize =
        std::env::var("TOPOS_DOUBLE_ECHO_COMMAND_CHANNEL_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048);
    pub static ref COMMAND_CHANNEL_CAPACITY: usize = COMMAND_CHANNEL_SIZE
        .checked_mul(10)
        .map(|v| {
            let r: usize = v.checked_div(100).unwrap_or(*COMMAND_CHANNEL_SIZE);
            r
        })
        .unwrap_or(*COMMAND_CHANNEL_SIZE);
    pub static ref TOPOS_DOUBLE_ECHO_MAX_BUFFER_SIZE: usize =
        std::env::var("TOPOS_BROADCAST_MAX_BUFFER_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(crate::double_echo::DoubleEcho::MAX_BUFFER_SIZE);
}
