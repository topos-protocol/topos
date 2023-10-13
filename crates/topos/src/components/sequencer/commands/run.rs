use clap::Args;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Args, Debug, Serialize)]
#[command(about = "Run a full Topos Sequencer instance")]
pub struct Run {
    /// SubnetId of the local subnet node, hex encoded 32 bytes starting with 0x
    #[clap(long, env = "TOPOS_LOCAL_SUBNET_ID")]
    pub subnet_id: Option<String>,

    /// Subnet endpoint in the form [ip address]:[port]
    /// Topos sequencer expects both websocket and http protocol available
    /// on this subnet endpoint. If optional `subnet_jsonrpc_ws` is not provided websocket endpoint
    /// will be deduced from this parameter.
    #[clap(
        long,
        default_value = "127.0.0.1:8545",
        env = "TOPOS_SUBNET_JSONRPC_HTTP"
    )]
    pub subnet_jsonrpc_http: String,

    /// Optional explicit websocket endpoint for the subnet jsonrpc api. If this parameter is not provided,
    /// it will be derived from the `subnet_jsonrpc_http`.
    /// Full uri value is expected, e.g. `wss://arbitrum.infura.com/v3/ws/mykey` or `ws://127.0.0.1/ws`
    #[clap(long, env = "TOPOS_SUBNET_JSONRPC_WS")]
    pub subnet_jsonrpc_ws: Option<String>,

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

    /// Start synchronizing from particular block number
    /// Default is to sync from genesis block (0)
    #[arg(long, env = "TOPOS_START_BLOCK")]
    pub start_block: Option<u64>,
}

impl Run {}
