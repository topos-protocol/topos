use opentelemetry::sdk::metrics::controllers::BasicController;
use opentelemetry::sdk::trace::{BatchConfig, BatchSpanProcessor, SpanLimits};
use opentelemetry::sdk::{propagation::TraceContextPropagator, trace::Sampler, Resource};
use opentelemetry::trace::TracerProvider;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{SpanExporterBuilder, WithExportConfig};
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
        _ => Level::TRACE,
    }
}

fn build_resources(otlp_service_name: String, version: &'static str) -> Vec<KeyValue> {
    let mut resources = Vec::new();

    resources.push(KeyValue::new("service.name", otlp_service_name));
    resources.push(KeyValue::new("service.version", version));

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

fn create_filter(verbose: u8) -> EnvFilter {
    if verbose > 0 {
        EnvFilter::try_new(format!("warn,topos={}", verbose_to_level(verbose).as_str())).unwrap()
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,topos=info"))
    }
}

// Setup tracing
// If otlp agent and otlp service name are provided, opentelemetry collection will be used
pub fn setup_tracing(
    verbose: u8,
    no_color: bool,
    otlp_agent: Option<String>,
    otlp_service_name: Option<String>,
    version: &'static str,
) -> Result<Option<BasicController>, Box<dyn std::error::Error>> {
    let mut layers = Vec::new();

    let ansi = !no_color;

    layers.push(
        match std::env::var("TOPOS_LOG_FORMAT")
            .map(|f| f.to_lowercase())
            .as_ref()
            .map(|s| s.as_str())
        {
            Ok("json") => tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(ansi)
                .with_filter(create_filter(verbose))
                .boxed(),
            Ok("pretty") => tracing_subscriber::fmt::layer()
                .pretty()
                .with_ansi(ansi)
                .with_filter(create_filter(verbose))
                .boxed(),
            _ => tracing_subscriber::fmt::layer()
                .compact()
                .with_ansi(ansi)
                .with_filter(create_filter(verbose))
                .boxed(),
        },
    );

    // Setup instrumentation if both otlp agent and otlp service name are provided as arguments
    let metrics: Option<_> = if let (Some(otlp_agent), Some(otlp_service_name)) =
        (otlp_agent, otlp_service_name)
    {
        let resources = build_resources(otlp_service_name, version);

        let mut trace_config = opentelemetry::sdk::trace::config();

        trace_config = trace_config.with_sampler(Sampler::AlwaysOn);
        trace_config = trace_config.with_max_events_per_span(
            match std::env::var("OTLP_MAX_EVENTS_PER_SPAN") {
                Ok(v) => v
                    .parse::<u32>()
                    .unwrap_or(SpanLimits::default().max_events_per_span),
                _ => SpanLimits::default().max_events_per_span,
            },
        );
        trace_config = trace_config.with_max_attributes_per_span(
            match std::env::var("OTLP_MAX_ATTRIBUTES_PER_SPAN") {
                Ok(v) => v
                    .parse::<u32>()
                    .unwrap_or(SpanLimits::default().max_attributes_per_span),
                _ => SpanLimits::default().max_attributes_per_span,
            },
        );
        trace_config =
            trace_config.with_max_links_per_span(match std::env::var("OTLP_MAX_LINK_PER_SPAN") {
                Ok(v) => v
                    .parse::<u32>()
                    .unwrap_or(SpanLimits::default().max_links_per_span),
                _ => SpanLimits::default().max_links_per_span,
            });
        trace_config = trace_config.with_max_attributes_per_event(
            match std::env::var("OTLP_MAX_ATTRIBUTES_PER_EVENT") {
                Ok(v) => v
                    .parse::<u32>()
                    .unwrap_or(SpanLimits::default().max_attributes_per_event),
                _ => SpanLimits::default().max_attributes_per_event,
            },
        );

        trace_config = trace_config.with_max_attributes_per_link(
            match std::env::var("OTLP_MAX_ATTRIBUTES_PER_LINK") {
                Ok(v) => v
                    .parse::<u32>()
                    .unwrap_or(SpanLimits::default().max_attributes_per_link),
                _ => SpanLimits::default().max_attributes_per_link,
            },
        );

        trace_config = trace_config.with_resource(Resource::new(resources));

        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_env()
            .with_endpoint(otlp_agent);

        let batch_processor_config = BatchConfig::default()
            .with_scheduled_delay(match std::env::var("OTLP_BATCH_SCHEDULED_DELAY") {
                Ok(v) => Duration::from_millis(v.parse::<u64>().unwrap_or(5_000)),
                _ => Duration::from_millis(5_000),
            })
            .with_max_queue_size(match std::env::var("OTLP_BATCH_MAX_QUEUE_SIZE") {
                Ok(v) => v.parse::<usize>().unwrap_or(2048),
                _ => 2048,
            })
            .with_max_export_batch_size(match std::env::var("OTLP_BATCH_MAX_EXPORTER_BATCH_SIZE") {
                Ok(v) => v.parse::<usize>().unwrap_or(512),
                _ => 512,
            })
            .with_max_export_timeout(match std::env::var("OTLP_BATCH_EXPORT_TIMEOUT") {
                Ok(v) => Duration::from_millis(v.parse::<u64>().unwrap_or(30_000)),
                _ => Duration::from_millis(30_000),
            })
            .with_max_concurrent_exports(
                match std::env::var("OTLP_BATCH_MAX_CONCURRENT_EXPORTS") {
                    Ok(v) => v.parse::<usize>().unwrap_or(1),
                    _ => 1,
                },
            );

        let span_exporter: SpanExporterBuilder = exporter.into();
        let mut provider_builder = opentelemetry::sdk::trace::TracerProvider::builder()
            .with_span_processor(
                BatchSpanProcessor::builder(
                    span_exporter.build_span_exporter().unwrap(),
                    opentelemetry::runtime::Tokio,
                )
                .with_batch_config(batch_processor_config)
                .build(),
            );

        provider_builder = provider_builder.with_config(trace_config);
        let provider = provider_builder.build();

        let tracer =
            provider.versioned_tracer("opentelemetry-otlp", Some(env!("CARGO_PKG_VERSION")), None);

        let _ = global::set_tracer_provider(provider);

        layers.push(
            tracing_opentelemetry::layer()
                .with_tracer(tracer)
                .with_filter(create_filter(verbose))
                .boxed(),
        );

        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

        global::set_text_map_propagator(TraceContextPropagator::new());

        None
    } else {
        None
    };

    tracing_subscriber::registry().with(layers).try_init()?;

    Ok(metrics)
}
