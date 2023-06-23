use clap::Args;
use serde::Serialize;
use std::net::SocketAddr;

use crate::options::input_format::InputFormat;

#[derive(Args, Clone, Debug, Serialize)]
pub(crate) struct PushCertificate {
    #[arg(short, long="format", value_enum, default_value_t = InputFormat::Plain)]
    pub(crate) format: InputFormat,

    /// Global timeout for the command
    #[arg(short, long = "timeout", default_value_t = 60)]
    pub(crate) timeout: u64,

    /// Seconds to wait before asserting the broadcast
    #[arg(long = "timeout-broadcast", default_value_t = 30)]
    pub(crate) timeout_broadcast: u64,

    /// The node list to be used, can be a file path or a comma separated list of Uri. If
    /// not provided, stdin is listened.
    #[arg(short, long = "nodes", env = "TARGET_NODES_PATH")]
    pub(crate) nodes: Option<String>,
}
