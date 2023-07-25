use lazy_static::lazy_static;

lazy_static! {
    /// Size of the double echo command channel
    pub static ref COMMAND_CHANNEL_SIZE: usize =
        std::env::var("TOPOS_DOUBLE_ECHO_COMMAND_CHANNEL_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048);
    /// Size of the channel between double echo and the task manager
    pub static ref TASK_MANAGER_CHANNEL_SIZE: usize =
        std::env::var("TOPOS_TASK_MANAGER_CHANNEL_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20_480);
    /// Size of the channel to send protocol events from the double echo
    pub static ref PROTOCOL_CHANNEL_SIZE: usize =
        std::env::var("TOPOS_PROTOCOL_CHANNEL_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048);
    /// Size of the channel to send updated subscriptions views to the double echo
    pub static ref SUBSCRIPTION_VIEW_CHANNEL_SIZE: usize =
        std::env::var("TOPOS_SUBSCRIPTION_VIEW_CHANNEL_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048);
    /// Capacity alert threshold for the double echo command channel
    pub static ref COMMAND_CHANNEL_CAPACITY: usize = COMMAND_CHANNEL_SIZE
        .checked_mul(10)
        .map(|v| {
            let r: usize = v.checked_div(100).unwrap_or(*COMMAND_CHANNEL_SIZE);
            r
        })
        .unwrap_or(*COMMAND_CHANNEL_SIZE);
    /// Size of the double echo buffer
    pub static ref TOPOS_DOUBLE_ECHO_MAX_BUFFER_SIZE: usize =
        std::env::var("TOPOS_BROADCAST_MAX_BUFFER_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(crate::double_echo::DoubleEcho::MAX_BUFFER_SIZE);
}
