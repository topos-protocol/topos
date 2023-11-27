use std::path::Path;
use std::{net::SocketAddr, path::PathBuf};

use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use topos_p2p::{Multiaddr, PeerId};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TceConfig {
    /// Storage database path, if not set RAM storage is used
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    /// Boot peers for the p2p network
    pub boot_peers: String,
    /// Ip for the p2p Multiaddr
    pub tce_ext_host: Option<String>,
    /// Port for the p2p Multiaddr
    pub tce_local_port: Option<u16>,
    /// Local peer secret key seed (optional, used for testing)
    pub local_key_seed: Option<String>,
    /// Connection degree for the GossipSub overlay
    pub minimum_tce_cluster_size: Option<usize>,
    /// gRPC API Addr
    #[serde(default = "default_libp2p_api_addr")]
    pub libp2p_api_addr: SocketAddr,
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

const fn default_libp2p_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(0, 0, 0, 0),
        9090,
    ))
}

const fn default_grpc_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(0, 0, 0, 0),
        1340,
    ))
}

const fn default_graphql_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(0, 0, 0, 0),
        4030,
    ))
}

const fn default_metrics_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(0, 0, 0, 0),
        3000,
    ))
}

impl TceConfig {
    pub fn parse_boot_peers(&self) -> Vec<(PeerId, Multiaddr)> {
        self.boot_peers
            .clone()
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
    type Output = TceConfig;

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

    fn profile() -> String {
        "tce".to_string()
    }
}
