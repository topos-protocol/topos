use clap::Args;
use serde::Serialize;

#[derive(Args, Clone, Debug, Serialize)]
pub(crate) struct Keys {
    #[arg(long = "from-seed")]
    pub(crate) from_seed: Option<String>,
}
