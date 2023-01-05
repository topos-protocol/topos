use clap::{Args, Subcommand};

mod peer_id;
mod push_peer_list;
mod run;

pub(crate) use push_peer_list::PushPeerList;
pub(crate) use run::Run;

use self::peer_id::PeerId;

#[derive(Args, Debug)]
pub(crate) struct TceCommand {
    #[clap(
        global = true,
        short,
        long = "endpoint",
        default_value = "http://[::1]:1340"
    )]
    pub(crate) endpoint: Option<String>,

    #[clap(subcommand)]
    pub(crate) subcommands: Option<TceCommands>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum TceCommands {
    PushPeerList(PushPeerList),
    PeerId(PeerId),
    Run(Box<Run>),
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
