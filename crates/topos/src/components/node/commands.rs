use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

mod init;
mod status;
mod up;

pub(crate) use init::Init;
pub(crate) use status::Status;
pub(crate) use up::Up;

#[derive(Args, Debug, Serialize)]
pub(crate) struct NodeArgument {
    #[clap(short, long, default_value = "http://[::1]:1340")]
    pub(crate) node: String,
}

/// Utility to manage your nodes in the Topos network
#[derive(Args, Debug)]
pub(crate) struct NodeCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(from_global)]
    pub(crate) no_color: bool,

    #[clap(from_global)]
    pub(crate) home: PathBuf,

    /// Installation directory path for Polygon Edge binary
    #[arg(
        global = true,
        long,
        env = "TOPOS_POLYGON_EDGE_BIN_PATH",
        default_value = "."
    )]
    pub(crate) edge_path: PathBuf,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<NodeCommands>,
}

#[derive(Subcommand, Debug, Serialize)]
pub(crate) enum NodeCommands {
    Up(Box<Up>),
    Init(Box<Init>),
    Status(Status),
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
