use opentelemetry::runtime;
use opentelemetry::sdk::{
    export::metrics::aggregation::cumulative_temporality_selector,
    metrics::selectors,
    propagation::TraceContextPropagator,
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{ExportConfig, Protocol, WithExportConfig};
use std::time::Duration;
use tracing::Level;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

fn verbose_to_level(verbose: u8) -> Level {
    match verbose {
        0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        4..=std::u8::MAX => Level::TRACE,
    }
}

fn build_resources(otlp_service_name: String) -> Vec<KeyValue> {
    let mut resources = Vec::new();

    resources.push(KeyValue::new("service.name", otlp_service_name));
    resources.push(KeyValue::new("service.version", env!("TOPOS_VERSION")));

    let custom_resources: Vec<_> = std::env::var("TOPOS_OTLP_TAGS")
        .unwrap_or_default()
        .split(',')
        // NOTE: limit to 10 tags to avoid exploit
        .take(10)
        .filter_map(|tag_raw| {
            let mut v = tag_raw.splitn(2, '=');
            match (v.next(), v.next()) {
                (Some(key), Some(value)) if !key.trim().is_empty() && !value.trim().is_empty() => {
                    Some(KeyValue::new(
                        key.trim().to_string(),
                        value.trim().to_string(),
                    ))
                }
                _ => None,
            }
        })
        .collect();

    resources.extend(custom_resources);

    resources
}

// Setup tracing
// If otlp agent and otlp service name are provided, opentelemetry collection will be used
pub(crate) fn setup_tracing(
    verbose: u8,
    otlp_agent: Option<String>,
    otlp_service_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let filter = if verbose > 0 {
        EnvFilter::try_new(format!("warn,topos={}", verbose_to_level(verbose).as_str())).unwrap()
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,topos=info"))
    };

    let mut layers = Vec::new();

    layers.push(
        match std::env::var("TOPOS_LOG_FORMAT")
            .map(|f| f.to_lowercase())
            .as_ref()
            .map(|s| s.as_str())
        {
            Ok("json") => tracing_subscriber::fmt::layer()
                .json()
                .with_filter(filter)
                .boxed(),
            Ok("pretty") => tracing_subscriber::fmt::layer()
                .pretty()
                .with_filter(filter)
                .boxed(),
            _ => tracing_subscriber::fmt::layer()
                .compact()
                .with_filter(filter)
                .boxed(),
        },
    );

    // Setup instrumentation if both otlp agent and otlp service name are provided as arguments
    if let (Some(otlp_agent), Some(otlp_service_name)) = (otlp_agent, otlp_service_name) {
        let resources = build_resources(otlp_service_name);

        global::set_text_map_propagator(TraceContextPropagator::new());
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(otlp_agent.clone()),
            )
            .with_trace_config(
                trace::config()
                    .with_sampler(Sampler::AlwaysOn)
                    .with_id_generator(RandomIdGenerator::default())
                    .with_max_events_per_span(64)
                    .with_max_attributes_per_span(16)
                    .with_max_events_per_span(16)
                    // resources will translated to tags in otlp spans
                    .with_resource(Resource::new(resources)),
            )
            .install_batch(opentelemetry::runtime::Tokio)
            .unwrap();

        let export_config = ExportConfig {
            endpoint: otlp_agent,
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

        layers.push(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(EnvFilter::try_new("topos=debug").unwrap())
                .boxed(),
        );
    }

    tracing_subscriber::registry().with(layers).try_init()?;

    Ok(())
}
