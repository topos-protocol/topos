use clap::{Args, Subcommand};

mod spam;

pub(crate) use spam::Spam;

#[derive(Args, Debug)]
pub(crate) struct NetworkCommand {
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
