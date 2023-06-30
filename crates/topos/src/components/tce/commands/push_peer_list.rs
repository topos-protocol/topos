use crate::options::input_format::InputFormat;
use clap::Args;

use super::NodeArgument;

#[derive(Args, Debug)]
pub(crate) struct PushPeerList {
    #[command(flatten)]
    pub(crate) node_args: NodeArgument,

    #[arg(short, long="format", value_enum, default_value_t = InputFormat::Plain)]
    pub(crate) format: InputFormat,

    #[arg(long)]
    pub(crate) force: bool,

    /// The peer ids list to be pushed, can be a file path or a comma separated list of PeerId. If
    /// not provided, stdin is listened.
    pub(crate) peers: Option<String>,
}
