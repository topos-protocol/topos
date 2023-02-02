#![allow(unused_imports)]
use std::time::Duration;

use clap::Parser;
#[cfg(feature = "tce")]
use components::tce::commands::{TceCommand, TceCommands};
use opentelemetry::sdk::propagation::TraceContextPropagator;
use opentelemetry::sdk::trace::{self, RandomIdGenerator, Sampler};
use opentelemetry::sdk::Resource;
use opentelemetry::{global, KeyValue};
use tracing::Level;
use tracing_subscriber::Layer;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

use opentelemetry::runtime;
use opentelemetry::sdk::export::metrics::aggregation::cumulative_temporality_selector;
use opentelemetry::sdk::metrics::selectors;
use opentelemetry_otlp::{ExportConfig, Protocol, WithExportConfig};

use tracing_log::LogTracer;

mod components;
mod options;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    LogTracer::init()?;

    let args = options::Opt::parse();

    let mut layers = Vec::new();

    let filter = if args.verbose > 0 {
        EnvFilter::try_new(format!("topos={}", verbose_to_level(args.verbose).as_str())).unwrap()
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("topos=info"))
    };

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

    #[cfg(feature = "tce")]
    if let options::ToposCommand::Tce(TceCommand {
        subcommands: Some(TceCommands::Run(ref cmd)),
        ..
    }) = args.commands
    {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(cmd.jaeger_agent.clone()),
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
                        KeyValue::new(
                            "service.name",
                            std::env::var("TCE_JAEGER_SERVICE_NAME")
                                .expect("TCE_JAEGER_SERVICE_NAME must be defined"),
                        ),
                        KeyValue::new("service.version", env!("TOPOS_VERSION")),
                    ])),
            )
            .install_batch(opentelemetry::runtime::Tokio)
            .unwrap();

        let export_config = ExportConfig {
            endpoint: cmd.jaeger_agent.clone(),
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

        layers.push(tracing_opentelemetry::layer().with_tracer(tracer).boxed());
    };

    tracing_subscriber::registry().with(layers).try_init()?;

    match args.commands {
        #[cfg(feature = "tce")]
        options::ToposCommand::Tce(cmd) => components::tce::handle_command(cmd).await,
        #[cfg(feature = "sequencer")]
        options::ToposCommand::Sequencer(cmd) => components::sequencer::handle_command(cmd).await,
    }
}

fn verbose_to_level(verbose: u8) -> Level {
    match verbose {
        0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        4..=std::u8::MAX => Level::TRACE,
    }
}
