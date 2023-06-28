use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

#[derive(Args, Debug, Serialize)]
#[command(about = "Spawn your node!")]
pub struct Up {
    /// Name to identify your node
    #[arg(long, env = "TOPOS_NODE_NAME", default_value = "default")]
    pub name: Option<String>,
}

impl Up {}
