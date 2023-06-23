use super::NodeArgument;
use clap::Args;
use serde::Serialize;

#[derive(Args, Clone, Debug, Serialize)]
pub(crate) struct Status {
    #[command(flatten)]
    pub(crate) node_args: NodeArgument,

    #[arg(long)]
    pub(crate) sample: bool,
}
