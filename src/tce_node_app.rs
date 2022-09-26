mod app_context;
mod cli;
mod storage;

use crate::app_context::AppContext;
use crate::cli::AppArgs;
use clap::Parser;
use libp2p::{identity, Multiaddr};
use tce_store::{Store, StoreConfig};
use tokio::spawn;
use topos_p2p::{utils::local_key_pair, Multiaddr, PeerId};
use topos_tce_broadcast::mem_store::TrbMemStore;
use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use tracing::info;

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    info!("Initializing application");
    let args = AppArgs::parse();

    tce_telemetry::init_tracer(&args.jaeger_agent, "testnet");

    // launch data store
    info!(
        "Storage: {}",
        if let Some(db_path) = args.db_path.clone() {
            format!("RocksDB: {}", &db_path)
        } else {
            "RAM".to_string()
        }
    );
    let config = ReliableBroadcastConfig {
        store: if let Some(db_path) = args.db_path.clone() {
            // Use RocksDB
            Box::new(Store::new(StoreConfig { db_path }))
        } else {
            // Use in RAM storage
            Box::new(TrbMemStore::new(Vec::new()))
        },
        trbp_params: args.trbp_params.clone(),
        my_peer_id: "main".to_string(),
    };

    info!("Starting application");
    let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.tce_local_port)
        .parse()
        .unwrap();
    // run protocol
    let (trbp_cli, trb_stream) = ReliableBroadcastClient::new(config);

    let (api_client, api_stream) = topos_tce_api::Runtime::builder().build_and_launch().await;

    let (network_client, event_stream, runtime) = topos_p2p::network::builder()
        .peer_key(local_key_pair(args.local_key_seed))
        .listen_addr(addr)
        .known_peers(args.parse_boot_peers())
        .build()
        .await
        .expect("Can't create network system");

    spawn(runtime.run());

    // setup transport-trbp-storage-api connector
    let app_context = AppContext::new(
        storage::inmemory::InmemoryStorage::default(),
        trbp_cli,
        network_client,
        api_client,
    );
    app_context.run(event_stream, trb_stream, api_stream).await;
}

/// build peer_id keys, generate for now - either from the seed or purely random one
fn local_key_pair(secret_key_seed: Option<u8>) -> identity::Keypair {
    // todo: load from protobuf encoded|base64 encoded config.local_key_pair
    match secret_key_seed {
        Some(seed) => {
            let mut bytes = [0u8; 32];
            bytes[0] = seed;
            let secret_key = identity::ed25519::SecretKey::from_bytes(&mut bytes).expect(
                "this returns `Err` only if the length is wrong; the length is correct; qed",
            );
            identity::Keypair::Ed25519(secret_key.into())
        }
        None => identity::Keypair::generate_ed25519(),
    }
}
