use crate::config::node::NodeConfig;
use crate::edge::{CommandConfig, BINARY_NAME};
use opentelemetry::global;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::str::FromStr;
use std::time::Duration;
use tokio::process::Command;
use tokio::{
    signal, spawn,
    sync::{mpsc, oneshot},
};
use topos_p2p::config::NetworkConfig;
use topos_sequencer::SequencerConfiguration;
use topos_tce::config::{AuthKey, StorageConfiguration, TceConfiguration};
use topos_tce_transport::ReliableBroadcastParams;
use topos_wallet::SecretManager;
use tracing::{error, info};

pub(crate) fn spawn_sequencer_process(config: NodeConfig, keys: &SecretManager) {
    let config = SequencerConfiguration {
        subnet_id: None,
        public_key: keys.validator_pubkey(),
        subnet_jsonrpc_endpoint: config.sequencer.subnet_jsonrpc_endpoint,
        subnet_contract_address: config.sequencer.subnet_contract_address,
        // TODO: Merge with or default to config.tce.tce_local_port?
        base_tce_api_url: config.sequencer.base_tce_api_url,
        signing_key: keys.validator.clone().unwrap(),
        verifier: config.sequencer.verifier,
    };

    spawn(async move {
        topos_sequencer::run(config, shutdown).await.map_err(|e| {
            error!("Failure on the Sequencer: {e:?}");
            Errors::SequencerFailure
        })
    })
}

pub(crate) fn spawn_tce_process(config: NodeConfig, keys: SecretManager) {
    let tce_config = TceConfiguration {
        boot_peers: config.parse_boot_peers(),
        auth_key: keys.network.map(AuthKey::PrivateKey),
        tce_addr: config.tce_ext_host,
        tce_local_port: config.tce_local_port,
        tce_params: ReliableBroadcastParams {
            echo_threshold: config.echo_threshold,
            ready_threshold: config.ready_threshold,
            delivery_threshold: config.delivery_threshold,
        },
        api_addr: config.tce.api_addr,
        graphql_api_addr: config.tce.graphql_api_addr,
        metrics_api_addr: config.tce.metrics_api_addr,
        storage: StorageConfiguration::RocksDB(PathBuf::from_str(&config.tce.db_path).ok()),
        network_bootstrap_timeout: Duration::from_secs(10),
        minimum_cluster_size: config
            .tce
            .minimum_tce_cluster_size
            .unwrap_or(NetworkConfig::MINIMUM_CLUSTER_SIZE),
        version: env!("TOPOS_VERSION"),
    };

    let (_shutdown_sender, shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);

    spawn(async move {
        if let Err(e) = topos_tce::run(&tce_config, shutdown_receiver).await {
            panic!("ok {e}");
        }
    });
}

pub fn spawn_edge_process(
    edge_path: PathBuf,
    data_dir: PathBuf,
    genesis_path: PathBuf,
) -> JoinHandle<Result<(), Errors>> {
    spawn(async move {
        match CommandConfig::new(edge_path)
            .server(&data_dir, &genesis_path)
            .spawn()
            .await
        {
            Ok(status) => {
                info!("Edge process terminated: {status:?}");
                Ok(())
            }
            Err(e) => Err(Errors::EdgeTerminated(e)),
        }
    })
}
