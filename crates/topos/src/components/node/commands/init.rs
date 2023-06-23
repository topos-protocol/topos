use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

#[derive(Args, Debug, Clone, Serialize)]
#[command(about = "Setup your node!")]
pub struct Init {
    /// Name to identify your node
    #[arg(long, env = "TOPOS_NODE_NAME")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Role of your node
    #[arg(long, env = "TOPOS_NODE_ROLE")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,

    /// Subnet of your node
    #[arg(long, env = "TOPOS_NODE_SUBNET")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet: Option<String>,
}

impl Init {}
