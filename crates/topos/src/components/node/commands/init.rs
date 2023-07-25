use std::path::PathBuf;

use crate::config::node::NodeRole;
use clap::Args;
use serde::Serialize;

#[derive(Args, Debug, Serialize)]
#[command(about = "Setup your node!", trailing_var_arg = true)]
pub struct Init {
    /// Name to identify your node
    #[arg(long, env = "TOPOS_NODE_NAME", default_value = "default")]
    pub name: Option<String>,

    /// Role of your node
    #[arg(long, value_enum, env = "TOPOS_NODE_ROLE", default_value_t = NodeRole::Validator)]
    pub role: NodeRole,

    /// Subnet of your node
    #[arg(long, env = "TOPOS_NODE_SUBNET", default_value = "topos")]
    pub subnet: Option<String>,
}
