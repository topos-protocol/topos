use clap::Args;

#[derive(Args, Debug)]
pub(crate) struct Status {
    #[arg(long)]
    pub(crate) sample: bool,
}
