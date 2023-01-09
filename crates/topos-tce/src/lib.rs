mod app_context;

use std::future::IntoFuture;
use std::net::SocketAddr;
use std::path::PathBuf;

pub use app_context::AppContext;
use opentelemetry::global;
use tce_transport::ReliableBroadcastParams;
use tokio::spawn;
use topos_p2p::utils::local_key_pair_from_slice;
use topos_p2p::{utils::local_key_pair, Multiaddr, PeerId};
use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_storage::{Connection, RocksDBStorage};

#[derive(Debug)]
pub struct TceConfiguration {
    pub local_key_seed: Option<Vec<u8>>,
    pub jaeger_agent: String,
    pub jaeger_service_name: String,
    pub tce_params: ReliableBroadcastParams,
    pub boot_peers: Vec<(PeerId, Multiaddr)>,
    pub api_addr: SocketAddr,
    pub tce_addr: String,
    pub tce_local_port: u16,
    pub storage: StorageConfiguration,
}

#[derive(Debug)]
pub enum StorageConfiguration {
    RAM,
    RocksDB(Option<PathBuf>),
}

pub async fn run(config: &TceConfiguration) -> Result<(), Box<dyn std::error::Error>> {
    let key = if let Some(seed) = &config.local_key_seed {
        local_key_pair_from_slice(seed)
    } else {
        local_key_pair(None)
    };

    let peer_id = key.public().to_peer_id();

    tracing::Span::current().record("peer_id", &peer_id.to_string());

    {
        // launch data store
        let tce_config = ReliableBroadcastConfig {
            tce_params: config.tce_params.clone(),
        };

        let (tce_cli, tce_stream) = ReliableBroadcastClient::new(tce_config);

        let (api_client, api_stream) = topos_tce_api::Runtime::builder()
            .serve_addr(config.api_addr)
            .build_and_launch()
            .await;

        let external_addr: Multiaddr = format!("{}/tcp/{}", config.tce_addr, config.tce_local_port)
            .parse()
            .unwrap();

        let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.tce_local_port)
            .parse()
            .unwrap();

        let (network_client, event_stream, runtime) = topos_p2p::network::builder()
            .peer_key(key)
            .listen_addr(addr)
            .exposed_addresses(external_addr)
            .known_peers(&config.boot_peers)
            .build()
            .await
            .expect("Can't create network system");

        spawn(runtime.run());

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

        let (gatekeeper_client, gatekeeper_runtime) = topos_tce_gatekeeper::Gatekeeper::builder()
            .local_peer_id(peer_id)
            .await
            .expect("Can't create the Gatekeeper");

        spawn(gatekeeper_runtime.into_future());

        let (synchronizer_client, synchronizer_runtime, synchronizer_stream) =
            topos_tce_synchronizer::Synchronizer::builder()
                .with_gatekeeper_client(gatekeeper_client.clone())
                .with_network_client(network_client.clone())
                .await
                .expect("Can't create the Synchronizer");

        spawn(synchronizer_runtime.into_future());

        // setup transport-tce-storage-api connector
        let app_context = AppContext::new(
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
            )
            .await;
    }

    global::shutdown_tracer_provider();
    Ok(())
}
