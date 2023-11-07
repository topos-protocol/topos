use std::path::PathBuf;

use clap::{Args, Subcommand};

mod init;
mod peer_id;
mod up;
mod status;

pub(crate) use init::Init;
pub(crate) use peer_id::PeerId;
pub(crate) use up::Up;
pub(crate) use status::Status;

#[derive(Args, Debug)]
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
    pub(crate) home: PathBuf,

    /// Installation directory path for Polygon Edge binary
    #[arg(long, env = "TOPOS_POLYGON_EDGE_BIN_PATH", default_value = ".")]
    pub(crate) edge_path: PathBuf,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<NodeCommands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum NodeCommands {
    Up(Box<Up>),
    Init(Box<Init>),
    PeerId(Box<PeerId>),
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
