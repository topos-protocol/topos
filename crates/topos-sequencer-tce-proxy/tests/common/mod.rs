use std::net::UdpSocket;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};
use topos_core::uci::SubnetId;
use topos_tce::{StorageConfiguration, TceConfiguration};
use topos_tce_storage::{RocksDBStorage, Storage};
use topos_tce_transport::ReliableBroadcastParams;
use tracing::info;

pub const SOURCE_SUBNET_ID_1: SubnetId = SubnetId::from_array([1u8; 32]);
pub const SOURCE_SUBNET_ID_2: SubnetId = SubnetId::from_array([2u8; 32]);
pub const TARGET_SUBNET_ID: SubnetId = SubnetId::from_array([3u8; 32]);
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

fn create_certificate_chain(
    source_subnet: topos_core::uci::SubnetId,
    target_subnet: topos_core::uci::SubnetId,
    number: usize,
) -> Vec<topos_core::uci::Certificate> {
    let mut certificates = Vec::new();
    let mut parent = None;

    for _ in 0..number {
        let cert = topos_core::uci::Certificate::new(
            parent.take().unwrap_or([0u8; 32]),
            source_subnet.clone(),
            Default::default(),
            Default::default(),
            &[target_subnet.clone()],
            0,
        )
        .unwrap();
        parent = Some(cert.id.as_array().clone());
        certificates.push(cert);
    }

    certificates
}

/// Populate database that will be used in test
pub async fn populate_test_database(
    rocksdb_dir: &PathBuf,
) -> Result<Vec<topos_core::uci::Certificate>, Box<dyn std::error::Error>> {
    info!("Populating test database storage in {rocksdb_dir:?}");

    let mut certificates = create_certificate_chain(
        SOURCE_SUBNET_ID_1,
        TARGET_SUBNET_ID,
        SOURCE_SUBNET_ID_1_NUMBER_OF_PREFILLED_CERTIFICATES,
    );
    certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_2,
        TARGET_SUBNET_ID,
        SOURCE_SUBNET_ID_2_NUMBER_OF_PREFILLED_CERTIFICATES,
    ));

    let storage = RocksDBStorage::with_isolation(&rocksdb_dir).expect("valid rocksdb storage");
    for certificate in &certificates {
        storage.persist(&certificate, None).await.unwrap();
    }

    info!("Finished populating test database");
    Ok(certificates)
}
