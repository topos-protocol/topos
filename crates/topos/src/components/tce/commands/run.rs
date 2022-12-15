use std::net::SocketAddr;

use clap::Args;
use topos_p2p::{Multiaddr, PeerId};
use topos_tce_transport::ReliableBroadcastParams;

#[derive(Args, Debug)]
#[command(about = "Run a full TCE instance")]
pub struct Run {
    /// Boot nodes to connect to, pairs of <PeerId> <Multiaddr>, space separated,
    /// quoted list like --boot-peers='a a1 b b1'
    #[arg(long, default_value = "", env = "TCE_BOOT_PEERS")]
    pub boot_peers: String,

    /// Advertised (externally visible) <host|address:port>,
    /// if empty this machine ip address(es) are used
    #[arg(long, env = "TCE_EXT_HOST")]
    pub tce_ext_host: Option<String>,

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
    pub local_key_seed: Option<u8>,

    /// Local peer key-pair (in base64 format)
    #[clap(long, env = "TCE_LOCAL_KEYPAIR")]
    pub local_key_pair: Option<String>,

    /// Storage database path, if not set RAM storage is used
    #[clap(long, default_value = "./target/db/", env = "TCE_DB_PATH")]
    pub db_path: Option<String>,

    /// Socket of the Jaeger agent endpoint
    #[clap(long, default_value = "127.0.0.1:6831", env = "TCE_JAEGER_AGENT")]
    pub jaeger_agent: String,

    /// Jaeger service name
    #[clap(
        long,
        default_value = "tce-jaeger-service",
        env = "TCE_JAEGER_SERVICE_NAME"
    )]
    pub jaeger_service_name: String,

    /// gRPC API Addr
    #[clap(long, env = "TCE_API_ADDR", default_value = "[::1]:1340")]
    pub api_addr: SocketAddr,

    /// TRBP parameters
    #[command(flatten)]
    pub trbp_params: ReliableBroadcastParams,
}

impl Run {
    pub fn parse_boot_peers(&self) -> Vec<(PeerId, Multiaddr)> {
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
