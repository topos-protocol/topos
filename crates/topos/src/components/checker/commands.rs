use clap::{Args, Subcommand};

use self::assert_delivery::AssertDelivery;

pub(crate) mod assert_delivery;

#[derive(Args, Debug)]
pub(crate) struct CheckerCommand {
    #[clap(subcommand)]
    pub(crate) subcommands: Option<CheckerCommands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum CheckerCommands {
    Tce(CheckerTceCommand),
}

#[derive(Args, Debug)]
pub(crate) struct CheckerTceCommand {
    #[clap(subcommand)]
    pub(crate) subcommands: Option<CheckerTceCommands>,
}

#[derive(Subcommand, Clone, Debug)]
pub(crate) enum CheckerTceCommands {
    AssertDelivery(AssertDelivery),
}
