use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

fn default_verifier() -> u32 {
    0
}

impl Default for SequencerConfig {
    fn default() -> Self {
        SequencerConfig {
            subnet_id: None,
            subnet_jsonrpc_endpoint: default_subnet_jsonrpc_endpoint(),
            subnet_contract_address: default_subnet_contract_address(),
            base_tce_api_url: default_base_tce_api_url(),
            subnet_data_dir: PathBuf::from("../test-chain-1"),
            verifier: default_verifier(),
            otlp_agent: None,
            otlp_service_name: None,
        }
    }
}
