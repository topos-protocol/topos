use std::path::Path;
use std::{net::SocketAddr, path::PathBuf};

use figment::providers::Serialized;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::config::Config;
use topos_p2p::{Multiaddr, PeerId};

const DEFAULT_IP: std::net::Ipv4Addr = std::net::Ipv4Addr::new(0, 0, 0, 0);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct TceConfig {
    /// Storage database path, if not set RAM storage is used
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    /// Array of extra boot nodes to connect to
    pub extra_boot_peers: Option<String>,
    /// Connection degree for the GossipSub overlay
    pub minimum_tce_cluster_size: Option<usize>,

    /// P2P configuration
    #[serde(default)]
    pub p2p: P2PConfig,

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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct P2PConfig {
    /// List of multiaddresses to listen for incoming connections
    #[serde(default = "default_listen_addresses")]
    pub listen_addresses: Vec<Multiaddr>,
    /// List of multiaddresses to advertise to the network
    #[serde(default = "default_advertised_addresses")]
    pub advertised_addresses: Vec<Multiaddr>,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            listen_addresses: default_listen_addresses(),
            advertised_addresses: default_advertised_addresses(),
        }
    }
}

fn default_db_path() -> PathBuf {
    PathBuf::from("./tce_rocksdb")
}

const fn default_libp2p_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(DEFAULT_IP, 9090))
}

fn default_listen_addresses() -> Vec<Multiaddr> {
    vec![format!(
        "/ip4/{}/tcp/{}",
        default_libp2p_api_addr().ip(),
        default_libp2p_api_addr().port()
    )
    .parse()
    .expect(
        r#"
        Listen multiaddresses generation failure.
        This is a critical bug that need to be report on `https://github.com/topos-protocol/topos/issues`
    "#,
    )]
}

fn default_advertised_addresses() -> Vec<Multiaddr> {
    vec![format!(
        "/ip4/{}/tcp/{}",
        default_libp2p_api_addr().ip(),
        default_libp2p_api_addr().port()
    )
    .parse()
    .expect(
        r#"
        Advertised multiaddresses generation failure.
        This is a critical bug that need to be report on `https://github.com/topos-protocol/topos/issues`
    "#,
    )]
}

const fn default_grpc_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(DEFAULT_IP, 1340))
}

const fn default_graphql_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(DEFAULT_IP, 4030))
}

const fn default_metrics_api_addr() -> SocketAddr {
    SocketAddr::V4(std::net::SocketAddrV4::new(DEFAULT_IP, 3000))
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
