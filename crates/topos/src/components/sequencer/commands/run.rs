use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;

#[derive(Parser, Debug, Clone, Serialize)]
#[command(about = "Run a full Topos Sequencer instance")]
pub struct Run {
    /// SubnetId of the local subnet node, hex encoded 32 bytes starting with 0x
    #[clap(long, env = "TOPOS_LOCAL_SUBNET_ID")]
    pub subnet_id: Option<String>,

    // Subnet endpoint in the form [ip address]:[port]
    // Topos sequencer expects both websocket and http protocol available
    // on this subnet endpoint
    #[clap(
        long,
        default_value = "127.0.0.1:8545",
        env = "SUBNET_JSONRPC_ENDPOINT"
    )]
    pub subnet_jsonrpc_endpoint: String,

    // Core contract address
    #[clap(long, env = "SUBNET_CONTRACT_ADDRESS")]
    pub subnet_contract_address: String,

    /// Base Uri of TCE node to call grpc service api
    #[clap(
        long,
        default_value = "http://[::1]:1340",
        env = "TOPOS_BASE_TCE_API_URL"
    )]
    pub base_tce_api_url: String,

    /// Polygon subnet node data dir, containing `consensus/validator.key`, e.g. `../test-chain-1`
    #[clap(long, env = "TOPOS_LOCAL_SUBNET_DATA_DIR")]
    pub subnet_data_dir: PathBuf,

    /// Verifier version
    #[clap(long, default_value = "0", env = "TOPOS_SEQUENCER_VERIFIER_VERSION")]
    pub verifier: u32,

    /// Socket of the opentelemetry agent endpoint
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_AGENT")]
    pub otlp_agent: Option<String>,

    /// Otlp service name
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_SERVICE_NAME")]
    pub otlp_service_name: Option<String>,
}

impl Run {}
