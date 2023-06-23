use clap::Parser;
use serde::Serialize;

use super::NodeArgument;

#[derive(Parser, Debug, Clone, Serialize)]
pub(crate) struct Status {
    #[command(flatten)]
    pub(crate) node_args: NodeArgument,

    #[arg(long)]
    pub(crate) sample: bool,
}
