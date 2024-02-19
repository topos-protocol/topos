use std::collections::HashSet;
use std::path::Path;
use std::{net::SocketAddr, path::PathBuf};

use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use topos_core::types::ValidatorId;
use topos_p2p::config::NetworkConfig;

use crate::Config;
use topos_p2p::{Multiaddr, PeerId};

use self::broadcast::ReliableBroadcastParams;
use self::p2p::P2PConfig;
use self::synchronization::SynchronizationConfig;

pub mod broadcast;
pub mod p2p;
pub mod synchronization;

const DEFAULT_IP: std::net::Ipv4Addr = std::net::Ipv4Addr::new(0, 0, 0, 0);

#[derive(Debug)]
pub enum AuthKey {
    Seed(Vec<u8>),
    PrivateKey(Vec<u8>),
}
#[derive(Default, Debug)]
pub enum StorageConfiguration {
    #[default]
    RAM,
    RocksDB(Option<PathBuf>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct TceConfig {
    #[serde(skip)]
    pub auth_key: Option<AuthKey>,
    #[serde(skip)]
    pub signing_key: Option<AuthKey>,
    #[serde(skip)]
    pub tce_params: ReliableBroadcastParams,
    #[serde(skip)]
    pub boot_peers: Vec<(PeerId, Multiaddr)>,
    #[serde(skip)]
    pub validators: HashSet<ValidatorId>,
    #[serde(skip)]
    pub storage: StorageConfiguration,

    #[serde(skip)]
    pub version: &'static str,

    /// Storage database path, if not set RAM storage is used
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
    /// Array of extra boot nodes to connect to
    pub extra_boot_peers: Option<String>,
    /// Connection degree for the GossipSub overlay
    #[serde(default = "default_minimum_tce_cluster_size")]
    pub minimum_tce_cluster_size: usize,

    /// libp2p addresses
    pub libp2p_api_addr: Option<SocketAddr>,

    /// P2P configuration
    #[serde(default)]
    pub p2p: P2PConfig,

    /// Synchronization configuration
    #[serde(default)]
    pub synchronization: SynchronizationConfig,

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

    #[serde(default = "default_network_bootstrap_timeout")]
    pub network_bootstrap_timeout: u64,
}

const fn default_network_bootstrap_timeout() -> u64 {
    90
}

fn default_db_path() -> PathBuf {
    PathBuf::from("./tce_rocksdb")
}
const fn default_minimum_tce_cluster_size() -> usize {
    NetworkConfig::MINIMUM_CLUSTER_SIZE
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
