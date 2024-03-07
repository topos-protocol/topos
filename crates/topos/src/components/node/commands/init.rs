use clap::Args;
use serde::Serialize;
use topos_config::node::NodeRole;

#[derive(Args, Debug, Serialize)]
#[command(about = "Setup your node", trailing_var_arg = true)]
#[serde(rename_all = "kebab-case")]
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

    /// The path to the SecretsManager config file. Used for Hashicorp Vault.
    /// If omitted, the local FS secrets manager is used
    #[arg(long, env = "TOPOS_SECRETS_MANAGER")]
    pub secrets_config: Option<String>,

    /// For certain use cases, we manually provide private keys to a running node, and don't want to
    /// rely on polygon-edge during runtime. Example: A Certificate Producer which runs for an external EVM chain
    #[arg(long, env = "TOPOS_NO_EDGE_PROCESS", action)]
    pub no_edge_process: bool,
}
