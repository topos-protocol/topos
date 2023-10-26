use super::NodeArgument;
use clap::Args;

#[derive(Args, Debug)]
pub(crate) struct Status {
    #[command(flatten)]
    pub(crate) node_args: NodeArgument,
}
