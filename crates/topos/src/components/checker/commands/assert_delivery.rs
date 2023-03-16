use clap::Args;

use crate::options::input_format::InputFormat;

#[derive(Args, Clone, Debug)]
pub(crate) struct AssertDelivery {
    #[arg(short, long="format", value_enum, default_value_t = InputFormat::Json)]
    pub(crate) format: InputFormat,

    /// Global timeout for the command
    #[arg(short, long = "timeout", default_value_t = 10)]
    pub(crate) timeout: u64,

    /// Seconds to wait before asserting the broadcast
    #[arg(long = "timeout-broadcast", default_value_t = 2)]
    pub(crate) timeout_broadcast: u64,

    /// The node list to be used, can be a file path or a comma separated list of Uri. If
    /// not provided, stdin is listened.
    #[arg(env = "TARGET_NODES_PATH")]
    pub(crate) peers: Option<String>,
}
