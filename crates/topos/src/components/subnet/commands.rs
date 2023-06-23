use clap::{Args, Subcommand};
use serde::Serialize;

mod subnet;

pub(crate) use subnet::Run;

/// Topos CLI subcommand for the Polygon Edge related functionalities
#[derive(Args, Debug, Clone, Serialize)]
pub(crate) struct SubnetCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<SubnetCommands>,
}

#[derive(Subcommand, Debug, Clone, Serialize)]
pub(crate) enum SubnetCommands {
    Run(Run),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        assert!(SubnetCommands::has_subcommand("run"));
    }
}
