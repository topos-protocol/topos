use clap::{Args, Subcommand};

mod push_peer_list;
mod run;

pub(crate) use push_peer_list::PushPeerList;
pub(crate) use run::Run;

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
    Run(Box<Run>),
}
