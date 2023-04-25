use clap::{Args, Subcommand};

mod subnet;

pub(crate) use subnet::Subnet;

/// Topos CLI subcommand used for setup of various Topos related components (e.g. installation of Polygon Edge binary)
#[derive(Args, Debug)]
pub(crate) struct SetupCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<SetupCommands>,
}

#[derive(Subcommand, Debug)]
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
