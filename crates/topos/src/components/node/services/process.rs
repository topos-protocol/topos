use crate::edge::CommandConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitStatus;
use thiserror::Error;
use tokio::{spawn, sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use topos_config::sequencer::SequencerConfig;
use topos_config::tce::{AuthKey, StorageConfiguration, TceConfig};
use topos_p2p::Multiaddr;
use topos_sequencer::SequencerConfiguration;
use topos_tce_transport::ReliableBroadcastParams;
use topos_wallet::SecretManager;
use tracing::{debug, error, info, warn};

use topos_config::genesis::Genesis;

#[derive(Error, Debug)]
pub enum Errors {
    #[error("TCE error")]
    TceFailure,
    #[error("Sequencer error")]
    SequencerFailure,
    #[error("Edge error: {0}")]
    EdgeTerminated(#[from] std::io::Error),
}

pub fn generate_edge_config(
    edge_path: PathBuf,
    config_path: PathBuf,
) -> JoinHandle<Result<ExitStatus, Errors>> {
    // Create the Polygon Edge config
    info!("Generating the configuration at {config_path:?}");
    info!("Polygon-edge binary located at: {edge_path:?}");
    spawn(async move {
        CommandConfig::new(edge_path)
            .init(&config_path)
            .spawn()
            .await
            .map_err(|e| {
                error!("Failed to generate the edge configuration: {e:?}");
                Errors::EdgeTerminated(e)
            })
    })
}

pub(crate) fn spawn_sequencer_process(
    config: SequencerConfig,
    keys: &SecretManager,
    shutdown: (CancellationToken, mpsc::Sender<()>),
) -> JoinHandle<Result<ExitStatus, Errors>> {
    let config = SequencerConfiguration {
        subnet_id: config.subnet_id,
        public_key: keys.validator_pubkey(),
        subnet_jsonrpc_http: config.subnet_jsonrpc_http,
        subnet_jsonrpc_ws: config.subnet_jsonrpc_ws,
        subnet_contract_address: config.subnet_contract_address,
        tce_grpc_endpoint: config.tce_grpc_endpoint,
        signing_key: keys.validator.clone().unwrap(),
        verifier: 0,
        start_block: config.start_block,
    };

    debug!("Sequencer args: {config:?}");
    spawn(async move {
        topos_sequencer::run(config, shutdown).await.map_err(|e| {
            error!("Sequencer failure: {e:?}");
            Errors::SequencerFailure
        })
    })
}

pub(crate) fn spawn_tce_process(
    mut config: TceConfig,
    keys: SecretManager,
    genesis: Genesis,
    shutdown: (CancellationToken, mpsc::Sender<()>),
) -> JoinHandle<Result<ExitStatus, Errors>> {
    config.boot_peers = genesis
        .boot_peers(Some(topos_p2p::constants::TCE_BOOTNODE_PORT))
        .into_iter()
        .chain(config.parse_boot_peers())
        .collect::<Vec<_>>();
    config.auth_key = keys.network.map(AuthKey::PrivateKey);
    config.signing_key = keys.validator.map(AuthKey::PrivateKey);
    config.p2p.is_bootnode = if let Some(AuthKey::PrivateKey(ref k)) = config.auth_key {
        let peer_id = topos_p2p::utils::keypair_from_protobuf_encoding(&k[..])
            .public()
            .to_peer_id();

        config.boot_peers.iter().any(|(p, _)| p == &peer_id)
    } else {
        false
    };

    config.validators = genesis.validators().expect("Cannot parse validators");
    config.tce_params = ReliableBroadcastParams::new(config.validators.len());

    if let Some(socket) = config.libp2p_api_addr {
        warn!(
            "`libp2p_api_addr` is deprecated in favor of `listen_addresses` and \
             `public_addresses` and will be removed in the next version. In order to keep your \
             node running, `libp2p_api_addr` will be used."
        );

        let addr: Multiaddr = format!("/ip4/{}/tcp/{}", socket.ip(), socket.port())
            .parse()
            .expect("Unable to generate Multiaddr from `libp2p_api_addr`");

        config.p2p.listen_addresses = vec![addr.clone()];
        config.p2p.public_addresses = vec![addr];
    }

    config.version = env!("TOPOS_VERSION");
    config.storage = StorageConfiguration::RocksDB(Some(config.db_path.clone()));

    debug!("TCE args: {config:?}");
    spawn(async move {
        topos_tce::run(&config, shutdown).await.map_err(|e| {
            error!("TCE process terminated: {e:?}");
            Errors::TceFailure
        })
    })
}

pub fn spawn_edge_process(
    edge_path: PathBuf,
    data_dir: PathBuf,
    genesis_path: PathBuf,
    edge_args: HashMap<String, String>,
) -> JoinHandle<Result<ExitStatus, Errors>> {
    spawn(async move {
        CommandConfig::new(edge_path)
            .server(&data_dir, &genesis_path, edge_args)
            .spawn()
            .await
            .map_err(Errors::EdgeTerminated)
    })
}
