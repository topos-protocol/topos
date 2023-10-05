use std::path::PathBuf;

use clap::{Args, Subcommand};

mod peer_id;
mod push_certificate;
mod run;
mod status;

pub(crate) use push_certificate::PushCertificate;
pub(crate) use run::Run;
pub(crate) use status::Status;

use self::peer_id::Keys;

#[derive(Args, Debug)]
pub(crate) struct NodeArgument {
    #[clap(short, long, default_value = "http://[::1]:1340")]
    pub(crate) node: String,
}

/// Topos CLI subcommand for the TCE related functionalities
#[derive(Args, Debug)]
pub(crate) struct TceCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(from_global)]
    pub(crate) home: PathBuf,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<TceCommands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum TceCommands {
    PushCertificate(PushCertificate),
    Keys(Keys),
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
