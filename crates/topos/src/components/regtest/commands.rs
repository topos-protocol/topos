use clap::{Args, Subcommand};

mod spam;

pub(crate) use spam::Spam;

/// Run test commands (e.g., pushing a certificate to a TCE process)
#[derive(Args, Debug)]
pub(crate) struct RegtestCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<RegtestCommands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum RegtestCommands {
    Spam(Box<Spam>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        assert!(RegtestCommands::has_subcommand("spam"));
    }
}
