use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(about = "Run a full Topos Sequencer instance")]
pub struct Run {
    /// SubnetId of the local subnet node
    #[clap(
        long,
        default_value = "0000000000000000000000000000000000000000000000000000000000000000",
        env = "TOPOS_LOCAL_SUBNET_ID"
    )]
    pub subnet_id: String,

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

    /// Keystore file
    #[clap(
        long,
        default_value = "topos-sequencer-keystore.json",
        env = "TOPOS_SEQUENCER_KEYSTORE_FILE"
    )]
    pub keystore_file: PathBuf,

    /// Keystore file password. If this parameter is not provided
    /// password prompt will be opened
    #[clap(long, env = "TOPOS_SEQUENCER_KEYSTORE_PASSWORD")]
    pub keystore_password: Option<String>,

    /// Verifier version
    #[clap(long, default_value = "0", env = "TOPOS_SEQUENCER_VERIFIER_VERSION")]
    pub verifier: u32,
}

impl Run {}
