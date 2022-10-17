mod app_context;
mod cli;

use std::{future::IntoFuture, path::PathBuf};

use crate::app_context::AppContext;
use crate::cli::AppArgs;
use clap::Parser;
use futures::FutureExt;
use tokio::spawn;
use topos_p2p::{utils::local_key_pair, Multiaddr};
use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_storage::{Connection, RocksDBStorage};
use tracing::info;

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    info!("Initializing application");
    let args = AppArgs::parse();

    tce_telemetry::init_tracer(&args.jaeger_agent, &args.jaeger_service_name);

    // launch data store
    info!(
        "Storage: {}",
        if let Some(db_path) = args.db_path.clone() {
            format!("RocksDB: {}", &db_path)
        } else {
            "RAM".to_string()
        }
    );
    let path = args.db_path.clone().unwrap();

    let storage = async move {
        let path = PathBuf::from(path);

        RocksDBStorage::open(&path).await
    };

    let (connection, store, _) = Connection::build(storage.boxed());

    let config = ReliableBroadcastConfig {
        store: store.clone(),
        trbp_params: args.trbp_params.clone(),
        my_peer_id: "main".to_string(),
    };

    info!("Starting application");
    spawn(connection.into_future());

    let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.tce_local_port)
        .parse()
        .unwrap();

    // run protocol
    let (trbp_cli, trb_stream) = ReliableBroadcastClient::new(config);

    let (api_client, api_stream) = topos_tce_api::Runtime::builder()
        .serve_addr(args.api_addr)
        .build_and_launch()
        .await;

    let (network_client, event_stream, runtime) = topos_p2p::network::builder()
        .peer_key(local_key_pair(args.local_key_seed))
        .listen_addr(addr)
        .known_peers(args.parse_boot_peers())
        .build()
        .await
        .expect("Can't create network system");

    spawn(runtime.run());

    // setup transport-trbp-storage-api connector
    let app_context = AppContext::new(store, trbp_cli, network_client, api_client);
    app_context.run(event_stream, trb_stream, api_stream).await;
}
