use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use tce_transport::ReliableBroadcastParams;
use topos_core::types::ValidatorId;
use topos_p2p::{Multiaddr, PeerId};

pub use crate::AppContext;

#[derive(Debug)]
pub enum AuthKey {
    Seed(Vec<u8>),
    PrivateKey(Vec<u8>),
}

#[derive(Debug)]
pub struct TceConfiguration {
    pub auth_key: Option<AuthKey>,
    pub signing_key: Option<AuthKey>,
    pub tce_params: ReliableBroadcastParams,
    pub boot_peers: Vec<(PeerId, Multiaddr)>,
    pub validators: HashSet<ValidatorId>,
    pub api_addr: SocketAddr,
    pub graphql_api_addr: SocketAddr,
    pub metrics_api_addr: SocketAddr,
    pub storage: StorageConfiguration,
    pub network_bootstrap_timeout: Duration,
    pub minimum_cluster_size: usize,
    pub version: &'static str,
    pub listen_addresses: Vec<Multiaddr>,
    pub advertised_addresses: Vec<Multiaddr>,
}

#[derive(Debug)]
pub enum StorageConfiguration {
    RAM,
    RocksDB(Option<PathBuf>),
}
