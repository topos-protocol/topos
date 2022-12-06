use clap::{Parser, Subcommand};

use crate::components::tce::commands::TceCommand;

pub(crate) mod input_format;

#[derive(Parser, Debug)]
#[clap(name = "topos", about = "Topos CLI")]
pub(crate) struct Opt {
    #[arg(
        long,
        short = 'v',
        action = clap::ArgAction::Count,
        global = true
    )]
    pub(crate) verbose: Option<u8>,

    #[command(subcommand)]
    pub(crate) commands: ToposCommand,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ToposCommand {
    Tce(TceCommand),
}
