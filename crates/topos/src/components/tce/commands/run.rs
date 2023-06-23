use std::any::type_name;
use std::net::SocketAddr;
use std::str::FromStr;

use clap::{Args, Parser};
use serde::{Deserialize, Serialize};

use topos_tce_transport::ReliableBroadcastParams;

#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(about = "Run a full TCE instance")]
pub struct Run {
    /// Boot nodes to connect to, pairs of <PeerId> <Multiaddr>, space separated,
    /// quoted list like --boot-peers='a a1,b b1'
    #[arg(long, env = "TCE_BOOT_PEERS")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_peers: Option<String>,

    /// Advertised (externally visible) <host>,
    /// if empty this machine ip address(es) are used
    #[arg(long, env = "TCE_EXT_HOST")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tce_ext_host: Option<String>,

    /// Port to listen on (host is 0.0.0.0, should be good for most installations)
    #[arg(long, env = "TCE_PORT")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tce_local_port: Option<u16>,

    /// WebAPI external url <host|address:port> (optional)
    #[arg(long, env = "TCE_WEB_API_EXT_URL")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_api_ext_url: Option<String>,

    /// WebAPI port
    #[arg(long, env = "TCE_WEB_API_PORT")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_api_local_port: Option<u16>,

    /// Local peer secret key seed (optional, used for testing)
    #[arg(long, env = "TCE_LOCAL_KS")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_key_seed: Option<String>,

    /// Local peer key-pair (in base64 format)
    #[arg(long, env = "TCE_LOCAL_KEYPAIR")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_key_pair: Option<String>,

    /// Storage database path, if not set RAM storage is used
    #[arg(long, env = "TCE_DB_PATH")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_path: Option<String>,

    /// gRPC API Addr
    #[arg(long, env = "TCE_API_ADDR")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_addr: Option<SocketAddr>,

    /// GraphQL API Addr
    #[arg(long, env = "TCE_GRAPHQL_API_ADDR")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphql_api_addr: Option<SocketAddr>,

    /// Broadcast parameters
    #[command(flatten)]
    pub tce_params: ReliableBroadcastParams,

    /// Socket of the opentelemetry agent endpoint
    /// If not provided open telemetry will not be used
    #[clap(long, env = "TOPOS_OTLP_AGENT")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otlp_agent: Option<String>,

    /// Otlp service name
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_SERVICE_NAME")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otlp_service_name: Option<String>,

    #[arg(long, env = "TOPOS_MINIMUM_TCE_CLUSTER_SIZE")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_tce_cluster_size: Option<usize>,
}
