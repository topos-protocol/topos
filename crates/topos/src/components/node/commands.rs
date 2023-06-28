use std::path::PathBuf;

use clap::{Args, Subcommand};

mod init;
mod up;

pub(crate) use init::Init;
pub(crate) use up::Up;

/// Utility to manage your nodes in the Topos network
#[derive(Args, Debug)]
pub(crate) struct NodeCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(from_global)]
    pub(crate) home: PathBuf,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<NodeCommands>,
}

#[derive(Subcommand, Debug)]
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
