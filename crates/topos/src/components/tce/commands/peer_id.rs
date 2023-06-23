use clap::Parser;
use serde::Serialize;

#[derive(Parser, Debug, Clone, Serialize)]
pub(crate) struct Keys {
    #[arg(long = "from-seed")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) from_seed: Option<String>,
}
