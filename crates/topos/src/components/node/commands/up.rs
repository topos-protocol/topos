use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

#[derive(Args, Debug, Serialize)]
#[command(about = "Spawn your node!")]
pub struct Up {
    /// Name to identify your node
    #[arg(long, env = "TOPOS_NODE_NAME", default_value = "default")]
    pub name: Option<String>,

    /// The path to the SecretsManager config file. Used for Hashicorp Vault.
    /// If omitted, the local FS secrets manager is used
    #[arg(long, env = "TOPOS_SECRETS_MANAGER")]
    pub secrets_config: Option<String>,
}
