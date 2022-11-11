mod app_context;
mod cli;
mod storage;

use crate::app_context::AppContext;
use crate::cli::AppArgs;
use clap::Parser;
use opentelemetry::sdk::trace::{self, RandomIdGenerator, Sampler};
use opentelemetry::sdk::Resource;
use opentelemetry::{global, KeyValue};
use tce_store::{Store, StoreConfig};
use tokio::spawn;
use topos_p2p::{utils::local_key_pair, Multiaddr};
use topos_tce_broadcast::mem_store::TrbMemStore;
use topos_tce_broadcast::{ReliableBroadcastClient, ReliableBroadcastConfig};
use tracing::{info, instrument, Instrument, Span};

use tracing_subscriber::{prelude::*, EnvFilter};

#[tokio::main]
#[instrument(name = "TCE", fields(peer_id))]
async fn main() {
    let args = AppArgs::parse();
    let key = local_key_pair(args.local_key_seed);
    let peer_id = key.public().to_peer_id();
    tracing::Span::current().record("peer_id", &peer_id.to_string());

    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_endpoint(args.jaeger_agent.clone())
        .with_service_name(args.jaeger_service_name.clone())
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
        .try_init()
        .unwrap();
    {
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

        let (trbp_cli, trb_stream) = ReliableBroadcastClient::new(config);

        let (api_client, api_stream) = topos_tce_api::Runtime::builder()
            .serve_addr(args.api_addr)
            .build_and_launch()
            .instrument(Span::current())
            .await;

        let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.tce_local_port)
            .parse()
            .unwrap();

        let (network_client, event_stream, runtime) = topos_p2p::network::builder()
            .peer_key(key)
            .listen_addr(addr)
            .known_peers(Span::current().in_scope(|| args.parse_boot_peers()))
            .build()
            .instrument(Span::current())
            .await
            .expect("Can't create network system");

        spawn(runtime.run().instrument(Span::current()));

        // setup transport-trbp-storage-api connector
        let app_context = AppContext::new(
            storage::inmemory::InmemoryStorage::default(),
            trbp_cli,
            network_client,
            api_client,
        );

        app_context.run(event_stream, trb_stream, api_stream).await;
    }
    global::shutdown_tracer_provider();
}
