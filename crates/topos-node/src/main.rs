use clap::Parser;
use libp2p::{Multiaddr, PeerId};

use topos_api::{ApiConfig, ApiWorker};
use topos_core_certification::CertificationWorker;
use topos_core_runtime_proxy::{RuntimeProxyConfig, RuntimeProxyWorker};
use topos_core_tce_proxy::{TceProxyConfig, TceProxyWorker};
use topos_net::{NetworkWorker, NetworkWorkerConfig};

use crate::app_context::AppContext;

mod app_context;

// use topos_store::{Store, StoreConfig};

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    log::info!("Starting application");

    let args = AppArgs::parse();

    // Launch the workers
    let certification = CertificationWorker::new(args.subnet_id);
    let runtime_proxy = RuntimeProxyWorker::new(RuntimeProxyConfig {
        endpoint: args.substrate_subnet_rpc_endpoint.clone(),
        topos_core_contract: args.topos_core_contract.clone(),
    });

    // downstream flow processor worker
    let tce_proxy_worker = TceProxyWorker::new(TceProxyConfig {
        subnet_id: args.subnet_id,
        base_tce_api_url: args.base_tce_api_url.clone(),
    });

    let api = ApiWorker::new(ApiConfig {
        web_port: args.web_api_local_port,
    });

    let srv = NetworkWorker::new(NetworkWorkerConfig {
        known_peers: args.parse_boot_peers(),
        local_port: args.topos_local_port,
        secret_key_seed: args.local_key_seed,
        local_key_pair: args.local_key_pair.clone(),
    });

    let mut app_context = AppContext::new(
        args.clone(),
        certification,
        runtime_proxy,
        api,
        srv,
        tce_proxy_worker,
    );
    app_context.run().await;
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
    #[clap(long, default_value_t = 0, env = "TOPOS_SUBNET_ID")]
    pub subnet_id: u64,

    // Subnet substrate interface rpc endpoint
    #[clap(
        long,
        default_value = "ws://127.0.0.1:9944",
        env = "TOPOS_SUBSTRATE_SUBNET_RPC_ENDPOINT"
    )]
    pub substrate_subnet_rpc_endpoint: String,

    // Topos core contract Eth address
    #[clap(long, env = "TOPOS_CORE_CONTRACT")]
    pub topos_core_contract: String,

    /// Base Uri of TCE node to call API at
    #[clap(
        long,
        default_value = "http://localhost:8080",
        env = "TOPOS_BASE_TCE_API_URL"
    )]
    pub base_tce_api_url: String,
}

impl AppArgs {
    pub fn parse_boot_peers(&self) -> Vec<(PeerId, Multiaddr)> {
        log::info!("boot_peers: {:?}", self.boot_peers);
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
