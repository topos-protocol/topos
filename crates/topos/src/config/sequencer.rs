use std::path::Path;

use crate::components::sequencer::commands::Run;
use crate::config::Config;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct SequencerConfig {
    /// SubnetId of your Sequencer, hex encoded 32 bytes prefixed with 0x
    pub subnet_id: Option<String>,

    /// JSON-RPC endpoint of the Edge node, websocket and http support expected
    /// If the endpoint address starts with `https`, ssl will be used with http/websocket
    #[serde(default = "default_subnet_jsonrpc_endpoint")]
    pub subnet_jsonrpc_endpoint: String,

    /// Address where the Topos Core contract is deployed
    #[serde(default = "default_subnet_contract_address")]
    pub subnet_contract_address: String,

    /// gRPC API endpoint of one TCE process
    #[serde(default = "default_tce_grpc_endpoint")]
    pub tce_grpc_endpoint: String,

    /// OTLP agent endpoint, not used if not provided
    pub otlp_agent: Option<String>,

    /// OTLP service name, not used if not provided
    pub otlp_service_name: Option<String>,
}

fn default_subnet_jsonrpc_endpoint() -> String {
    "127.0.0.1:8545".to_string()
}

fn default_subnet_contract_address() -> String {
    "0x0000000000000000000000000000000000000000".to_string()
}

fn default_tce_grpc_endpoint() -> String {
    "http://[::1]:1340".to_string()
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
