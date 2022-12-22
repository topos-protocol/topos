use clap::Parser;
use topos_p2p::{Multiaddr, PeerId};

use crate::app_context::AppContext;
use topos_sequencer_api::{ApiConfig, ApiWorker};
use topos_sequencer_certification::CertificationWorker;
use topos_sequencer_subnet_runtime_proxy::{RuntimeProxyConfig, RuntimeProxyWorker};
use topos_sequencer_tce_proxy::{TceProxyConfig, TceProxyWorker};
use tracing::info;
use tracing_subscriber::{prelude::*, EnvFilter};
use topos_sequencer_types::SubnetId;

mod app_context;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "log-json")]
    let formatting_layer = tracing_subscriber::fmt::layer().json();

    #[cfg(not(feature = "log-json"))]
    let formatting_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap())
        .with(formatting_layer)
        .set_default();

    info!("Starting topos-sequencer application");

    let mut args = AppArgs::parse();

    let pass = args.topos_node_keystore_password.take().unwrap_or_else(|| {
        rpassword::prompt_password("Keystore password:").expect("Valid keystore password")
    });

    let subnet_id: SubnetId = hex::decode(&args.subnet_id)?.as_slice().try_into()?;

    // Launch the workers
    let certification = CertificationWorker::new(subnet_id)?;

    let runtime_proxy = RuntimeProxyWorker::new(RuntimeProxyConfig {
        subnet_id,
        endpoint: args.subnet_jsonrpc_endpoint.clone(),
        subnet_contract: args.subnet_contract.clone(),
        keystore_file: args.topos_node_keystore_file.clone(),
        keystore_password: pass,
    })?;

    // downstream flow processor worker
    let tce_proxy_worker = TceProxyWorker::new(TceProxyConfig {
        subnet_id,
        base_tce_api_url: args.base_tce_api_url.clone(),
    })
    .await?;

    let api = ApiWorker::new(ApiConfig {
        web_port: args.web_api_local_port,
    });

    let mut app_context = AppContext::new(
        args.clone(),
        certification,
        runtime_proxy,
        api,
        tce_proxy_worker,
    );
    app_context.run().await
}

/// Application configuration
#[derive(Debug, Parser, Clone)]
#[clap(name = "Topos Node (toposware.com)")]
pub struct AppArgs {
    /// Boot nodes to connect to, pairs of <PeerId> <Multiaddr>, space separated,
    /// quoted list like --boot-peers='a a1 b b1'
    #[clap(long, default_value = "", env = "TCE_BOOT_PEERS")]
    pub boot_peers: String,
    /// Advertised (externally visible) <host|address:port>,
    /// if empty this machine ip address(es) are used
    #[clap(long, env = "TOPOS_EXT_HOST")]
    pub topos_ext_host: Option<String>,
    /// Port to listen on (host is 0.0.0.0, should be good for most installations)
    #[clap(long, default_value_t = 0, env = "TOPOS_PORT")]
    pub topos_local_port: u16,
    /// WebAPI external url <host|address:port> (optional)
    #[clap(long, env = "TOPOS_WEB_API_EXT_URL")]
    pub web_api_ext_url: Option<String>,
    /// WebAPI port
    #[clap(long, default_value_t = 8080, env = "TOPOS_WEB_API_PORT")]
    pub web_api_local_port: u16,
    /// Local peer secret key seed (optional, used for testing)
    #[clap(long, env = "TOPOS_LOCAL_KS")]
    pub local_key_seed: Option<u8>,
    /// Local peer key-pair (in base64 format)
    #[clap(long, env = "TOPOS_LOCAL_KEYPAIR")]
    pub local_key_pair: Option<String>,
    /// Storage database path, if not set current dir is used
    #[clap(long, env = "TOPOS_DB_PATH")]
    pub db_path: Option<String>,

    /// SubnetId to use
    #[clap(long, default_value_t = 0.to_string(), env = "TOPOS_SUBNET_ID")]
    pub subnet_id: String,

    // Subnet substrate interface rpc endpoint
    #[clap(
        long,
        default_value = "ws://127.0.0.1:8545/ws",
        env = "TOPOS_SUBNET_JSONRPC_ENDPOINT"
    )]
    pub subnet_jsonrpc_endpoint: String,

    // Ethereum core contract address
    #[clap(long, env = "SUBNET_CONTRACT")]
    pub subnet_contract: String,

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
        default_value = "topos-node-keystore.json",
        env = "TOPOS_NODE_KEYSTORE_FILE"
    )]
    pub topos_node_keystore_file: String,

    /// Keystore file password. If this parameter is not provided
    /// password prompt will be opened
    #[clap(long, env = "TOPOS_NODE_KEYSTORE_PASSWORD")]
    pub topos_node_keystore_password: Option<String>,
}

impl AppArgs {
    pub fn parse_boot_peers(&self) -> Vec<(PeerId, Multiaddr)> {
        info!("boot_peers: {:?}", self.boot_peers);
        self.boot_peers
            .split(' ')
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
            .chunks(2)
            .filter_map(|pair| {
                if pair.len() > 1 {
                    Some((
                        pair[0].as_str().parse().unwrap(),
                        pair[1].as_str().parse().unwrap(),
                    ))
                } else {
                    None
                }
            })
            .collect()
    }
}
