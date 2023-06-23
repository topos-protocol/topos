use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

#[derive(Args, Debug, Clone, Serialize)]
#[command(about = "Setup your node!")]
pub struct Init {
    /// Name to identify your node
    #[arg(long, env = "TOPOS_NODE_NAME", default_value = "default")]
    pub name: Option<String>,

    /// Role of your node
    #[arg(long, env = "TOPOS_NODE_ROLE", default_value = "validator")]
    pub role: Option<String>,

    /// Subnet of your node
    #[arg(long, env = "TOPOS_NODE_SUBNET", default_value = "topos")]
    pub subnet: Option<String>,
}

impl Init {}
