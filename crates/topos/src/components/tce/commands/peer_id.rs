use clap::Args;

#[derive(Args, Debug)]
pub(crate) struct Keys {
    #[arg(long = "from-seed")]
    pub(crate) from_seed: Option<String>,
}
