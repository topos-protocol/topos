use clap::Args;
use serde::Serialize;

#[derive(Args, Clone, Debug, Serialize)]
#[command(about = "Spawn your node")]
#[serde(rename_all = "kebab-case")]
pub struct Up {
    /// Name to identify your node
    #[arg(long, env = "TOPOS_NODE_NAME", default_value = "default")]
    pub name: Option<String>,

    /// The path to the SecretsManager config file. Used for Hashicorp Vault.
    /// If omitted, the local FS secrets manager is used
    #[arg(long, env = "TOPOS_SECRETS_MANAGER")]
    pub secrets_config: Option<String>,

    /// Defines that an external edge node will be use, replacing the one normally run by the node.
    /// Usable for cases where edge endpoint is available as infura (or similar cloud provider) endpoint
    #[arg(long, env = "TOPOS_NO_EDGE_PROCESS", action)]
    pub no_edge_process: bool,

    /// Socket of the opentelemetry agent endpoint.
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_AGENT")]
    pub otlp_agent: Option<String>,

    /// Otlp service name.
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_SERVICE_NAME")]
    pub otlp_service_name: Option<String>,
}
