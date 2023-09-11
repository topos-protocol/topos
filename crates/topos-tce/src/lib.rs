use std::future::IntoFuture;

use config::TceConfiguration;
use opentelemetry::global;
use tce_transport::AuthorityId;
use tokio::{spawn, sync::mpsc};
use tokio_util::sync::CancellationToken;
use topos_p2p::{
    utils::{local_key_pair, local_key_pair_from_slice},
    Multiaddr,
};
use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_storage::{Connection, RocksDBStorage};
use tracing::{debug, warn};
mod app_context;
pub mod config;
pub mod events;
pub mod messages;

pub use app_context::AppContext;

use crate::config::{AuthKey, StorageConfiguration};

pub async fn run(
    config: &TceConfiguration,
    shutdown: (CancellationToken, mpsc::Sender<()>),
) -> Result<(), Box<dyn std::error::Error>> {
    topos_metrics::init_metrics();

    let key = match config.auth_key.as_ref() {
        Some(AuthKey::Seed(seed)) => local_key_pair_from_slice(seed),
        Some(AuthKey::PrivateKey(pk)) => topos_p2p::utils::keypair_from_protobuf_encoding(pk),
        None => local_key_pair(None),
    };

    let peer_id = key.public().to_peer_id();
    let authority_id = AuthorityId::new(&key.public().try_into_secp256k1()?.to_bytes()[0..=20])?;

    warn!("I am {}", peer_id);

    tracing::Span::current().record("peer_id", &peer_id.to_string());

    let external_addr: Multiaddr =
        format!("{}/tcp/{}", config.tce_addr, config.tce_local_port).parse()?;

    let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.tce_local_port).parse()?;

    // Remove myself from the bootnode list
    let mut boot_peers = config.boot_peers.clone();
    boot_peers.retain(|(p, _)| *p != peer_id);

    let (network_client, event_stream, unbootstrapped_runtime) = topos_p2p::network::builder()
        .peer_key(key)
        .listen_addr(addr)
        .minimum_cluster_size(config.minimum_cluster_size)
        .exposed_addresses(external_addr)
        .known_peers(&boot_peers)
        .build()
        .await?;

    debug!("Starting the p2p network");
    let network_runtime = tokio::time::timeout(
        config.network_bootstrap_timeout,
        unbootstrapped_runtime.bootstrap(),
    )
    .await??;
    let _network_handler = spawn(network_runtime.run());
    debug!("p2p network started");

    debug!("Starting the gatekeeper");
    let (gatekeeper_client, gatekeeper_runtime) = topos_tce_gatekeeper::Gatekeeper::builder()
        .local_peer_id(peer_id)
        .await?;
    spawn(gatekeeper_runtime.into_future());
    debug!("Gatekeeper started");

    debug!("Starting the Storage");
    let (storage, storage_client, storage_stream) =
        if let StorageConfiguration::RocksDB(Some(ref path)) = config.storage {
            let storage = RocksDBStorage::open(path)?;
            Connection::build(Box::pin(async { Ok(storage) }))
        } else {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Unsupported storage type {:?}", config.storage),
            )));
        };
    spawn(storage.into_future());
    debug!("Storage started");

    debug!("Starting reliable broadcast");
    let (tce_cli, tce_stream) = ReliableBroadcastClient::new(ReliableBroadcastConfig {
        tce_params: config.tce_params.clone(),
        authority_id,
        validators: config.validators.clone(),
    })
    .await;
    debug!("Reliable broadcast started");

    debug!("Starting the Synchronizer");
    let (synchronizer_client, synchronizer_runtime, synchronizer_stream) =
        topos_tce_synchronizer::Synchronizer::builder()
            .with_gatekeeper_client(gatekeeper_client.clone())
            .with_network_client(network_client.clone())
            .await?;

    spawn(synchronizer_runtime.into_future());
    debug!("Synchronizer started");

    debug!("Starting gRPC api");
    let (api_client, api_stream, _ctx) = topos_tce_api::Runtime::builder()
        .with_peer_id(peer_id.to_string())
        .serve_grpc_addr(config.api_addr)
        .serve_graphql_addr(config.graphql_api_addr)
        .serve_metrics_addr(config.metrics_api_addr)
        .storage(storage_client.clone())
        .build_and_launch()
        .await;
    debug!("gRPC api started");

    // setup transport-tce-storage-api connector
    let (app_context, _tce_stream) = AppContext::new(
        storage_client,
        tce_cli,
        network_client,
        api_client,
        gatekeeper_client,
        synchronizer_client,
    );

    app_context
        .run(
            event_stream,
            tce_stream,
            api_stream,
            storage_stream,
            synchronizer_stream,
            shutdown,
        )
        .await;

    global::shutdown_tracer_provider();
    Ok(())
}
