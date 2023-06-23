use clap::{Args, Subcommand};
use serde::Serialize;

mod subnet;

pub(crate) use subnet::Subnet;

/// Topos CLI subcommand for the setup of various Topos related components (e.g., installation of Polygon Edge binary)
#[derive(Args, Debug, Clone, Serialize)]
pub(crate) struct SetupCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<SetupCommands>,
}

#[derive(Subcommand, Debug, Clone, Serialize)]
pub(crate) enum SetupCommands {
    Subnet(Box<Subnet>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        assert!(SetupCommands::has_subcommand("subnet"));
    }
}
