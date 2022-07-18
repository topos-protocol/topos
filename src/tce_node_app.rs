mod app_context;

use clap::Parser;

use tce_api::{ApiConfig, ApiWorker};
use tce_net::{NetworkWorker, NetworkWorkerConfig};
use tce_trbp::{ReliableBroadcastClient, ReliableBroadcastConfig};

use crate::app_context::AppContext;
use libp2p::{Multiaddr, PeerId};

use tce_store::{Store, StoreConfig};
use tce_transport::ReliableBroadcastParams;
use tce_trbp::mem_store::TrbMemStore;

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    log::info!("Initializing application");
    let args = AppArgs::parse();

    tce_telemetry::init_tracer(&args.jaeger_agent, "testnet");

    // launch data store
    log::info!(
        "Storage: {}",
        if let Some(db_path) = args.db_path.clone() {
            format!("RocksDB: {}", &db_path)
        } else {
            "RAM".to_string()
        }
    );
    let config = ReliableBroadcastConfig {
        store: if let Some(db_path) = args.db_path.clone() {
            // Use RocksDB
            Box::new(Store::new(StoreConfig { db_path }))
        } else {
            // Use in RAM storage
            Box::new(TrbMemStore::new(Vec::new()))
        },
        trbp_params: args.trbp_params.clone(),
        my_peer_id: "main".to_string(),
    };

    log::info!("Starting application");

    // run protocol
    let trbp_cli = ReliableBroadcastClient::new(config);

    // run APi services
    let api = ApiWorker::new(ApiConfig {
        web_port: args.web_api_local_port,
    });

    // run transport
    let srv = NetworkWorker::new(NetworkWorkerConfig {
        known_peers: args.parse_boot_peers(),
        local_port: args.tce_local_port,
        secret_key_seed: args.local_key_seed,
        local_key_pair: args.local_key_pair,
    });

    // setup transport-trbp-storage-api connector
    let mut app_context = AppContext::new(trbp_cli.clone(), api, srv);
    app_context.run().await;
}

/// Application configuration
#[derive(Debug, Parser)]
#[clap(name = "TCE node (toposware.com)")]
pub struct AppArgs {
    /// Boot nodes to connect to, pairs of <PeerId> <Multiaddr>, space separated,
    /// quoted list like --boot-peers='a a1 b b1'
    #[clap(long, default_value = "", env = "TCE_BOOT_PEERS")]
    pub boot_peers: String,
    /// Advertised (externally visible) <host|address:port>,
    /// if empty this machine ip address(es) are used
    #[clap(long, env = "TCE_EXT_HOST")]
    pub tce_ext_host: Option<String>,
    /// Port to listen on (host is 0.0.0.0, should be good for most installations)
    #[clap(long, default_value_t = 0, env = "TCE_PORT")]
    pub tce_local_port: u16,
    /// WebAPI external url <host|address:port> (optional)
    #[clap(long, env = "TCE_WEB_API_EXT_URL")]
    pub web_api_ext_url: Option<String>,
    /// WebAPI port
    #[clap(long, default_value_t = 8080, env = "TCE_WEB_API_PORT")]
    pub web_api_local_port: u16,
    /// Local peer secret key seed (optional, used for testing)
    #[clap(long, env = "TCE_LOCAL_KS")]
    pub local_key_seed: Option<u8>,
    /// Local peer key-pair (in base64 format)
    #[clap(long, env = "TCE_LOCAL_KEYPAIR")]
    pub local_key_pair: Option<String>,
    /// Storage database path, if not set RAM storage is used
    #[clap(long, env = "TCE_DB_PATH")]
    pub db_path: Option<String>,
    /// Socket of the Jaeger agent endpoint
    #[clap(long, default_value = "127.0.0.1:6831", env = "TCE_JAEGER_AGENT")]
    pub jaeger_agent: String,
    /// Testing only - deliver certificate immediately upon submission
    #[clap(long, env = "TCE_TEST_IMMEDIATE_DELIVERY")]
    pub test_immediate_delivery: bool,
    /// TRBP parameters
    #[clap(flatten)]
    pub trbp_params: ReliableBroadcastParams,
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
