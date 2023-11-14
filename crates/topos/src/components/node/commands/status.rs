use super::NodeArgument;
use clap::Args;
use serde::Serialize;

#[derive(Args, Debug, Serialize)]
#[command(about = "Get node status")]
pub(crate) struct Status {
    #[command(flatten)]
    pub(crate) node_args: NodeArgument,

    #[arg(long)]
    pub(crate) sample: bool,
}
