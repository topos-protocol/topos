use clap::Args;

#[derive(Args, Debug)]
pub(crate) struct PeerId {
    #[arg(long = "from-slice")]
    pub(crate) from_slice: Option<String>,
}
