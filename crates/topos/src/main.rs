#![allow(unused_imports)]
use std::time::Duration;

use clap::Parser;
use components::tce::commands::{TceCommand, TceCommands};
use opentelemetry::sdk::propagation::TraceContextPropagator;
use opentelemetry::sdk::trace::{self, RandomIdGenerator, Sampler};
use opentelemetry::sdk::Resource;
use opentelemetry::{global, KeyValue};
use tracing::Level;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

use opentelemetry::runtime;
use opentelemetry::sdk::export::metrics::aggregation::cumulative_temporality_selector;
use opentelemetry::sdk::metrics::selectors;
use opentelemetry_otlp::{ExportConfig, Protocol, WithExportConfig};

mod components;
mod options;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = options::Opt::parse();

    let level_filter = if args.verbose > 0 {
        EnvFilter::try_new(format!("topos={}", verbose_to_level(args.verbose).as_str())).unwrap()
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("topos=info"))
    };

    let log_format = std::env::var("TOPOS_LOG_FORMAT")
        .map(|f| f.to_lowercase())
        .ok();

    let json_format = log_format
        .as_ref()
        .filter(|format| format == &"json")
        .map(|_| tracing_subscriber::fmt::layer().json());

    let pretty_format = log_format
        .as_ref()
        .filter(|format| format == &"pretty")
        .map(|_| tracing_subscriber::fmt::layer().pretty());

    let default_fmt = if json_format.is_none() && pretty_format.is_none() {
        Some(tracing_subscriber::fmt::layer().compact())
    } else {
        None
    };

    let opentelemetry = if let options::ToposCommand::Tce(TceCommand {
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
            // .with_trace_config(
            //     trace::config()
            //         .with_sampler(Sampler)
            //         .with_id_generator(RandomIdGenerator::default())
            //         .with_max_events_per_span(64)
            //         .with_max_attributes_per_span(16)
            //         .with_max_events_per_span(16)
            //         // resources will translated to tags in jaeger spans
            //         .with_resource(Resource::new(vec![KeyValue::new("service.name", "topos")])),
            // )
            .install_batch(opentelemetry::runtime::Tokio)
            .unwrap();

        let _export_config = ExportConfig {
            endpoint: cmd.jaeger_agent.clone(),
            timeout: Duration::from_secs(3),
            protocol: Protocol::Grpc,
        };

        // let _meter = opentelemetry_otlp::new_pipeline()
        //     .metrics(
        //         selectors::simple::inexpensive(),
        //         cumulative_temporality_selector(),
        //         runtime::Tokio,
        //     )
        //     .with_exporter(
        //         opentelemetry_otlp::new_exporter()
        //             .tonic()
        //             .with_export_config(export_config),
        //     )
        //     .build();

        Some(tracing_opentelemetry::layer().with_tracer(tracer))
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(level_filter)
        .with(pretty_format)
        .with(json_format)
        .with(default_fmt)
        .with(opentelemetry)
        .try_init()?;

    match args.commands {
        options::ToposCommand::Tce(cmd) => components::tce::handle_command(cmd).await,
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
