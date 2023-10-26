use std::path::PathBuf;

use clap::{Args, Subcommand};

mod push_certificate;
mod run;
mod status;

pub(crate) use push_certificate::PushCertificate;
pub(crate) use run::Run;
pub(crate) use status::Status;

#[derive(Args, Debug)]
pub(crate) struct NodeArgument {
    #[clap(short, long, default_value = "http://[::1]:1340")]
    pub(crate) node: String,
}

/// Topos CLI subcommand for the TCE related functionalities
#[derive(Args, Debug)]
pub(crate) struct TceCommand {
    #[clap(subcommand)]
    pub(crate) subcommands: Option<TceCommands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum TceCommands {
    PushCertificate(PushCertificate),
    Run(Box<Run>),
    Status(Status),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        assert!(TceCommands::has_subcommand("run"));
    }
}
