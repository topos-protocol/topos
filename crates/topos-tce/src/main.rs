mod app_context;
mod cli;
mod storage;

use crate::app_context::AppContext;
use crate::cli::AppArgs;
use clap::Parser;
use opentelemetry::global;
use opentelemetry::runtime;
use opentelemetry::sdk::export::metrics::aggregation::cumulative_temporality_selector;
use opentelemetry::sdk::metrics::selectors;
use opentelemetry::sdk::trace::{self, RandomIdGenerator, Sampler};
use opentelemetry::sdk::Resource;
use opentelemetry::KeyValue;
use opentelemetry_otlp::{ExportConfig, Protocol, WithExportConfig};
use std::time::Duration;
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

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://otel-collector-opentelemetry-collector:4317"),
        )
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

    let export_config = ExportConfig {
        endpoint: "http://otel-collector-opentelemetry-collector:4317".to_string(),
        timeout: Duration::from_secs(3),
        protocol: Protocol::Grpc,
    };

    let _meter = opentelemetry_otlp::new_pipeline()
        .metrics(
            selectors::simple::inexpensive(),
            cumulative_temporality_selector(),
            runtime::Tokio,
        )
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_export_config(export_config),
        )
        .build();

    #[cfg(feature = "log-json")]
    let formatting_layer = tracing_subscriber::fmt::layer().json();

    #[cfg(not(feature = "log-json"))]
    let formatting_layer = tracing_subscriber::fmt::layer();

    // opentelemetry config
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
