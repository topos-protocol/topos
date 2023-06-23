use std::net::SocketAddr;

use clap::{Args, Subcommand};
use serde::Serialize;

mod peer_id;
mod push_certificate;
mod push_peer_list;
mod run;
mod status;

pub(crate) use push_certificate::PushCertificate;
pub(crate) use push_peer_list::PushPeerList;
pub(crate) use run::Run;
pub(crate) use status::Status;

use self::peer_id::Keys;

#[derive(Args, Debug, Clone, Serialize)]
pub(crate) struct NodeArgument {
    #[clap(short, long, default_value = "http://[::1]:1340")]
    pub(crate) node: String,
}

/// Topos CLI subcommand for the TCE related functionalities
#[derive(Args, Debug, Clone, Serialize)]
pub(crate) struct TceCommand {
    #[clap(from_global)]
    pub(crate) verbose: u8,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<TceCommands>,
}

#[derive(Subcommand, Debug, Clone, Serialize)]
pub(crate) enum TceCommands {
    PushCertificate(PushCertificate),
    PushPeerList(PushPeerList),
    Keys(Keys),
    Run(Box<Run>),
    Status(Status),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_peer_list() {
        assert!(TceCommands::has_subcommand("push-peer-list"));
    }

    #[test]
    fn test_run() {
        assert!(TceCommands::has_subcommand("run"));
    }
}
