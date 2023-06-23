use clap::{Args, Subcommand};
use serde::Serialize;

mod spam;

pub(crate) use spam::Spam;

/// Topos CLI subcommand for network related functionalities (e.g., running the certificate spammer)
#[derive(Args, Debug, Clone, Serialize)]
pub(crate) struct NetworkCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<NetworkCommands>,
}

#[derive(Subcommand, Debug, Clone, Serialize)]
pub(crate) enum NetworkCommands {
    Spam(Box<Spam>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        assert!(NetworkCommands::has_subcommand("spam"));
    }
}
