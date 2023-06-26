use std::path::{Path, PathBuf};

use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::components::sequencer::commands::Run;
use crate::config::Config;

#[derive(Serialize, Deserialize, Debug)]
pub struct SequencerConfig {
    /// SubnetId of the local subnet node, hex encoded 32 bytes starting with 0x
    pub subnet_id: Option<String>,

    // Subnet endpoint in the form [ip address]:[port]
    // Topos sequencer expects both websocket and http protocol available
    // on this subnet endpoint
    #[serde(default = "default_subnet_jsonrpc_endpoint")]
    pub subnet_jsonrpc_endpoint: String,

    // Core contract address
    #[serde(default = "default_subnet_contract_address")]
    pub subnet_contract_address: String,

    /// Base Uri of TCE node to call grpc service api
    #[serde(default = "default_base_tce_api_url")]
    pub base_tce_api_url: String,

    /// Polygon subnet node data dir, containing `consensus/validator.key`, e.g. `../test-chain-1`
    #[serde(default = "default_subnet_data_dir")]
    pub subnet_data_dir: PathBuf,

    /// Verifier version
    #[serde(default = "default_verifier")]
    pub verifier: u32,

    /// Socket of the opentelemetry agent endpoint
    /// If not provided open telemetry will not be used
    pub otlp_agent: Option<String>,

    /// Otlp service name
    /// If not provided open telemetry will not be used
    pub otlp_service_name: Option<String>,
}

fn default_subnet_jsonrpc_endpoint() -> String {
    "127.0.0.1:8545".to_string()
}

fn default_subnet_contract_address() -> String {
    "0x0000000000000000000000000000000000000000".to_string()
}

fn default_base_tce_api_url() -> String {
    "http://[::1]:1340".to_string()
}

fn default_subnet_data_dir() -> PathBuf {
    PathBuf::from("../test-chain-1")
}

fn default_verifier() -> u32 {
    0
}

impl Config for SequencerConfig {
    type Command = Run;

    type Output = Self;

    fn load_from_file(figment: Figment, home: &Path) -> Figment {
        let home = home.join("config.toml");

        let sequencer = Figment::new()
            .merge(Toml::file(home).nested())
            .select("sequencer");

        figment.merge(sequencer)
    }

    fn load_context(figment: Figment) -> Result<Self::Output, figment::Error> {
        figment.extract()
    }

    fn profile(&self) -> String {
        "sequencer".to_string()
    }
}
