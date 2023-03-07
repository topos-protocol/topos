use std::net::UdpSocket;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use topos_tce::{StorageConfiguration, TceConfiguration};
use topos_tce_transport::ReliableBroadcastParams;
use tracing::info;

pub const SOURCE_SUBNET_ID_1_NUMBER_OF_PREFILLED_CERTIFICATES: usize = 15;
pub const SOURCE_SUBNET_ID_2_NUMBER_OF_PREFILLED_CERTIFICATES: usize = 10;

/// Start test TCE node
/// Return task handle, shutdown channel and address
pub async fn start_tce_test_service(
    rocksdb_temp_dir: PathBuf,
) -> Result<(mpsc::Sender<oneshot::Sender<()>>, String), Box<dyn std::error::Error>> {
    info!("Starting test TCE node...");

    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let api_addr = socket.local_addr().ok().unwrap();

    let tce_address = api_addr.to_string();

    let config = TceConfiguration {
        boot_peers: vec![],
        local_key_seed: None,
        jaeger_agent: "http://otel-collector:12345".to_string(),
        jaeger_service_name: "topos-test".to_string(),
        tce_addr: "/ip4/0.0.0.0".to_string(),
        tce_local_port: 0,
        tce_params: ReliableBroadcastParams {
            echo_threshold: 1,
            echo_sample_size: 1,
            ready_threshold: 1,
            ready_sample_size: 1,
            delivery_threshold: 1,
            delivery_sample_size: 1,
        },
        api_addr,
        storage: StorageConfiguration::RocksDB(Some(rocksdb_temp_dir)),
        network_bootstrap_timeout: std::time::Duration::from_secs(2),
        version: "test",
    };

    let (shutdown_sender, shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);

    tokio::spawn(async move {
        if let Err(e) = topos_tce::run(&config, shutdown_receiver).await {
            panic!("TCE test node terminated with error {e}");
        };
    });

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    Ok((shutdown_sender, tce_address))
}
