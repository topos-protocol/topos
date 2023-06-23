use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

mod init;
mod up;

pub(crate) use init::Init;
pub(crate) use up::Up;

/// Utility to manage your nodes in the Topos network
#[derive(Args, Debug, Clone, Serialize)]
pub(crate) struct NodeCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    /// Home directory for the configuration
    #[arg(long, default_value = "~/.config/topos", env = "TOPOS_HOME")]
    pub home: PathBuf,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<NodeCommands>,
}

#[derive(Subcommand, Debug, Clone, Serialize)]
pub(crate) enum NodeCommands {
    Up(Box<Up>),
    Init(Box<Init>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        assert!(NodeCommands::has_subcommand("up"));
        assert!(NodeCommands::has_subcommand("init"));
    }
}
