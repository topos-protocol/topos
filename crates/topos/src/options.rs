use clap::{Parser, Subcommand};
use serde::Serialize;
use std::{ffi::OsString, path::PathBuf};

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

#[cfg(all(feature = "sequencer", feature = "tce"))]
use crate::components::node::commands::NodeCommand;

pub(crate) mod input_format;

#[derive(Parser, Debug, Serialize)]
#[clap(name = "topos", about = "Topos CLI")]
pub struct Opt {
    /// Defines the verbosity level
    #[arg(
        long,
        short = 'v',
        action = clap::ArgAction::Count,
        global = true
    )]
    pub(crate) verbose: u8,

    /// Home directory for the configuration
    #[arg(
    long,
    env = "TOPOS_HOME",
    default_value = get_default_home(),
    global = true
    )]
    pub(crate) home: PathBuf,

    #[command(subcommand)]
    pub(crate) commands: ToposCommand,
}

/// If no path is given for the --home argument, we use the default one
/// ~/.config/topos for a UNIX subsystem
fn get_default_home() -> OsString {
    let mut home = dirs::home_dir().unwrap();
    home.push(".config");
    home.push("topos");
    home.into_os_string()
}

#[derive(Subcommand, Debug, Clone, Serialize)]
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
    #[cfg(all(feature = "sequencer", feature = "tce"))]
    Node(NodeCommand),
    Doctor,
}
