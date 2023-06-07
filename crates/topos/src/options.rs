use clap::{Parser, Subcommand};

#[cfg(feature = "sequencer")]
use crate::components::sequencer::commands::SequencerCommand;

#[cfg(feature = "tce")]
use crate::components::tce::commands::TceCommand;

#[cfg(feature = "network")]
use crate::components::network::commands::NetworkCommand;

#[cfg(feature = "setup")]
use crate::components::setup::commands::SetupCommand;

#[cfg(feature = "subnet")]
use crate::components::subnet::commands::SubnetCommand;

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
    #[cfg(feature = "network")]
    Network(NetworkCommand),
    #[cfg(feature = "setup")]
    Setup(SetupCommand),
    #[cfg(feature = "subnet")]
    Subnet(SubnetCommand),
    Doctor,
}
