use clap::{Parser, Subcommand};

#[cfg(feature = "sequencer")]
use crate::components::sequencer::commands::SequencerCommand;

#[cfg(feature = "checker")]
use crate::components::checker::commands::CheckerCommand;

#[cfg(feature = "tce")]
use crate::components::tce::commands::TceCommand;

#[cfg(feature = "network")]
use crate::components::network::commands::NetworkCommand;

pub(crate) mod input_format;

#[derive(Parser, Debug)]
#[clap(name = "topos", about = "Topos CLI")]
pub(crate) struct Opt {
    /// Defines the verbosity level
    #[arg(
        long,
        short = 'v',
        action = clap::ArgAction::Count,
        global = true
    )]
    pub(crate) verbose: u8,

    #[command(subcommand)]
    pub(crate) commands: ToposCommand,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ToposCommand {
    #[cfg(feature = "tce")]
    Tce(TceCommand),
    #[cfg(feature = "sequencer")]
    Sequencer(SequencerCommand),
    #[cfg(feature = "checker")]
    Checker(CheckerCommand),
    #[cfg(feature = "network")]
    Network(NetworkCommand),
}
