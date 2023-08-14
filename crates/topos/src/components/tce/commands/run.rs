use clap::Args;
use serde::Serialize;
use std::net::SocketAddr;
use topos_p2p::{Multiaddr, PeerId};
use topos_tce_transport::ReliableBroadcastParams;

#[derive(Args, Debug, Serialize)]
#[command(about = "Run a full TCE instance")]
pub struct Run {
    /// Boot nodes to connect to, pairs of <PeerId> <Multiaddr>, space separated,
    /// quoted list like --boot-peers='a a1,b b1'
    #[arg(long, default_value = "", env = "TCE_BOOT_PEERS")]
    pub boot_peers: String,

    /// Advertised (externally visible) <host>,
    /// if empty this machine ip address(es) are used
    #[arg(long, env = "TCE_EXT_HOST", default_value = "/ip4/0.0.0.0")]
    pub tce_ext_host: String,

    /// Port to listen on (host is 0.0.0.0, should be good for most installations)
    #[arg(long, default_value_t = 0, env = "TCE_PORT")]
    pub tce_local_port: u16,

    /// WebAPI external url <host|address:port> (optional)
    #[clap(long, env = "TCE_WEB_API_EXT_URL")]
    pub web_api_ext_url: Option<String>,

    /// WebAPI port
    #[clap(long, default_value_t = 8080, env = "TCE_WEB_API_PORT")]
    pub web_api_local_port: u16,

    /// Local peer secret key seed (optional, used for testing)
    #[clap(long, env = "TCE_LOCAL_KS")]
    pub local_key_seed: Option<String>,

    /// Storage database path, if not set RAM storage is used
    #[clap(long, default_value = "./default_db/", env = "TCE_DB_PATH")]
    pub db_path: Option<String>,

    /// gRPC API Addr
    #[clap(long, env = "TCE_API_ADDR", default_value = "[::1]:1340")]
    pub api_addr: SocketAddr,

    /// GraphQL API Addr
    #[clap(long, env = "TCE_GRAPHQL_API_ADDR", default_value = "[::1]:4000")]
    pub graphql_api_addr: SocketAddr,

    /// Metrics server API Addr
    #[clap(long, env = "TCE_METRICS_API_ADDR", default_value = "[::1]:3000")]
    pub metrics_api_addr: SocketAddr,

    /// Broadcast parameters
    #[command(flatten)]
    pub tce_params: ReliableBroadcastParams,

    /// Socket of the opentelemetry agent endpoint
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_AGENT")]
    pub otlp_agent: Option<String>,

    /// Otlp service name
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_SERVICE_NAME")]
    pub otlp_service_name: Option<String>,

    #[arg(long, env = "TOPOS_MINIMUM_TCE_CLUSTER_SIZE")]
    pub minimum_tce_cluster_size: Option<usize>,
}

impl Run {
    pub fn parse_boot_peers(&self) -> Vec<(PeerId, Multiaddr)> {
        self.boot_peers
            .split(&[',', ' '])
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
