mod app_context;

use std::future::IntoFuture;
use std::net::SocketAddr;
use std::path::PathBuf;

pub use app_context::AppContext;
use opentelemetry::sdk::trace::{self, RandomIdGenerator, Sampler};
use opentelemetry::sdk::Resource;
use opentelemetry::{global, KeyValue};
use tce_transport::ReliableBroadcastParams;
use tokio::spawn;
use topos_p2p::{utils::local_key_pair, Multiaddr, PeerId};
use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use topos_tce_storage::{Connection, RocksDBStorage};
use tracing::{instrument, Instrument, Span};

use tracing_subscriber::{prelude::*, EnvFilter};

#[derive(Debug)]
pub struct TceConfiguration {
    pub local_key_seed: Option<u8>,
    pub jaeger_agent: String,
    pub jaeger_service_name: String,
    pub trbp_params: ReliableBroadcastParams,
    pub boot_peers: Vec<(PeerId, Multiaddr)>,
    pub api_addr: SocketAddr,
    pub tce_local_port: u16,
    pub storage: StorageConfiguration,
}

#[derive(Debug)]
pub enum StorageConfiguration {
    RAM,
    RocksDB(Option<PathBuf>),
}

#[instrument(name = "TCE", fields(peer_id), skip(config))]
pub async fn run(config: &TceConfiguration) -> Result<(), Box<dyn std::error::Error>> {
    let key = local_key_pair(config.local_key_seed);
    let peer_id = key.public().to_peer_id();

    tracing::Span::current().record("peer_id", &peer_id.to_string());

    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_endpoint(config.jaeger_agent.clone())
        .with_service_name(config.jaeger_service_name.clone())
        .with_max_packet_size(1500)
        .with_auto_split_batch(true)
        .with_instrumentation_library_tags(false)
        .with_trace_config(
            trace::config()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_max_events_per_span(64)
                .with_max_attributes_per_span(16)
                .with_max_events_per_span(16)
                // resources will translated to tags in jaeger spans
                .with_resource(Resource::new(vec![
                    KeyValue::new("key", "value"),
                    KeyValue::new("process_key", "process_value"),
                ])),
        )
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    #[cfg(feature = "log-json")]
    let formatting_layer = tracing_subscriber::fmt::layer().json();

    #[cfg(not(feature = "log-json"))]
    let formatting_layer = tracing_subscriber::fmt::layer();

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap())
        .with(formatting_layer)
        .with(opentelemetry)
        .set_default();

    {
        // launch data store
        let trb_config = ReliableBroadcastConfig {
            trbp_params: config.trbp_params.clone(),
            my_peer_id: "main".to_string(),
        };

        let (trbp_cli, trb_stream) = ReliableBroadcastClient::new(trb_config);

        let (api_client, api_stream) = topos_tce_api::Runtime::builder()
            .serve_addr(config.api_addr)
            .build_and_launch()
            .instrument(Span::current())
            .await;

        let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", config.tce_local_port)
            .parse()
            .unwrap();

        let (network_client, event_stream, runtime) = topos_p2p::network::builder()
            .peer_key(key)
            .listen_addr(addr)
            .known_peers(&config.boot_peers)
            .build()
            .instrument(Span::current())
            .await
            .expect("Can't create network system");

        spawn(runtime.run().instrument(Span::current()));

        let (storage, storage_client, storage_stream) =
            if let StorageConfiguration::RocksDB(Some(ref path)) = config.storage {
                let storage = RocksDBStorage::open(path)?;
                Connection::build(Box::pin(async { Ok(storage) }))
            } else {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unsupported storage type",
                )));
            };

        spawn(storage.into_future());

        // setup transport-trbp-storage-api connector
        let app_context = AppContext::new(storage_client, trbp_cli, network_client, api_client);

        app_context
            .run(event_stream, trb_stream, api_stream, storage_stream)
            .await;
    }

    global::shutdown_tracer_provider();
    Ok(())
}
