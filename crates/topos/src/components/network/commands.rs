use clap::{Args, Subcommand};

mod spam;

pub(crate) use spam::Spam;

/// Topos CLI subcommand for using network related functionality,(e.g. running a certificate spammer)
#[derive(Args, Debug)]
pub(crate) struct NetworkCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<NetworkCommands>,
}

#[derive(Subcommand, Debug)]
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
