use clap::{Args, Subcommand};

mod subnet;

pub(crate) use subnet::Run;

/// Topos CLI subcommand used to utilize subnet (Polygon Edge) related functionality
#[derive(Args, Debug)]
pub(crate) struct SubnetCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<SubnetCommands>,
}

#[derive(Subcommand, Debug)]
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
