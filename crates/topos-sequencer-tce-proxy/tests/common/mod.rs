use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::sync::{mpsc, oneshot};
use topos_tce::{StorageConfiguration, TceConfiguration};
use topos_tce_transport::ReliableBroadcastParams;
use tracing::info;

const TCE_LOCAL_API_ADDRESS: &str = "127.0.0.1:5001";

/// Start test TCE node
/// Return task handle, shutdown channel and address
pub async fn start_tce_test_service(
) -> Result<(mpsc::Sender<oneshot::Sender<()>>, String), Box<dyn std::error::Error>> {
    info!("Starting test TCE node...");
    let tce_address = TCE_LOCAL_API_ADDRESS.to_string();

    // Generate rocksdb path
    let mut rocksdb_temp_dir =
        PathBuf::from_str(env!("CARGO_TARGET_TMPDIR")).expect("Unable to read CARGO_TARGET_TMPDIR");
    rocksdb_temp_dir.push(format!(
        "./topos-sequencer-tce-proxy/data_{}/rocksdb",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid system time duration")
            .as_millis()
            .to_string()
    ));

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
        api_addr: SocketAddr::from_str(TCE_LOCAL_API_ADDRESS).unwrap(),
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

    Ok((shutdown_sender, tce_address.to_string()))
}
