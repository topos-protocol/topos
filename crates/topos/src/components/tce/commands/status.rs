use clap::Args;

use super::NodeArgument;

#[derive(Args, Debug)]
pub(crate) struct Status {
    #[command(flatten)]
    pub(crate) node_args: NodeArgument,

    #[arg(long)]
    pub(crate) sample: bool,
}
