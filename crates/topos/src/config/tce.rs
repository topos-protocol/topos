use std::path::Path;
use std::{net::SocketAddr, path::PathBuf};

use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::components::tce::commands::Run;
use crate::config::Config;
use topos_p2p::{Multiaddr, PeerId};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TceConfig {
    /// Storage database path, if not set RAM storage is used
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    /// Array of extra boot nodes to connect to
    pub extra_boot_peers: Option<String>,
    /// Ip for the p2p Multiaddr
    pub tce_ext_host: Option<String>,
    /// Port for the p2p Multiaddr
    pub tce_local_port: Option<u16>,
    /// Local peer secret key seed (optional, used for testing)
    pub local_key_seed: Option<String>,
    /// Connection degree for the GossipSub overlay
    pub minimum_tce_cluster_size: Option<usize>,
    /// gRPC API Addr
    #[serde(default = "default_grpc_api_addr")]
    pub grpc_api_addr: SocketAddr,
    /// GraphQL API Addr
    #[serde(default = "default_graphql_api_addr")]
    pub graphql_api_addr: SocketAddr,
    /// Metrics server API Addr
    #[serde(default = "default_metrics_api_addr")]
    pub metrics_api_addr: SocketAddr,
    /// Socket of the opentelemetry agent endpoint
    /// If not provided open telemetry will not be used
    pub otlp_agent: Option<String>,
    /// Otlp service name
    /// If not provided open telemetry will not be used
    pub otlp_service_name: Option<String>,
}

fn default_db_path() -> PathBuf {
    PathBuf::from("./tce_rocksdb")
}

fn default_grpc_api_addr() -> SocketAddr {
    "0.0.0.0:1340"
        .parse()
        .expect("Cannot parse address to SocketAddr")
}

fn default_graphql_api_addr() -> SocketAddr {
    "0.0.0.0:4030"
        .parse()
        .expect("Cannot parse address to SocketAddr")
}

fn default_metrics_api_addr() -> SocketAddr {
    "0.0.0.0:3000"
        .parse()
        .expect("Cannot parse address to SocketAddr")
}

impl TceConfig {
    pub fn parse_boot_peers(&self) -> Vec<(PeerId, Multiaddr)> {
        self.extra_boot_peers
            .clone()
            .unwrap_or_default()
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

impl Config for TceConfig {
    type Command = Run;
    type Output = TceConfig;

    fn profile(&self) -> String {
        "tce".to_string()
    }

    fn load_from_file(figment: Figment, home: &Path) -> Figment {
        let home = home.join("config.toml");

        let tce = Figment::new()
            .merge(Toml::file(home).nested())
            .select("tce");

        figment.merge(tce)
    }

    fn load_context(figment: Figment) -> Result<Self::Output, figment::Error> {
        figment.extract()
    }
}
