use clap::Parser;
use tracing::Level;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

mod components;
mod options;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = options::Opt::parse();

    let level_filter = if let Some(verbosity) = args.verbose {
        EnvFilter::try_new(format!("topos={}", verbose_to_level(verbosity).as_str())).unwrap()
    } else {
        EnvFilter::try_from_default_env().unwrap()
    };

    let tracing = tracing_subscriber::registry().with(level_filter);

    match std::env::var("TOPOS_LOG_FORMAT").map(|f| f.to_lowercase()) {
        Ok(format) if format == "json" => tracing
            .with(tracing_subscriber::fmt::layer().json())
            .try_init()
            .unwrap(),
        Ok(format) if format == "pretty" => tracing
            .with(tracing_subscriber::fmt::layer().pretty())
            .try_init()
            .unwrap(),
        _ => tracing
            .with(tracing_subscriber::fmt::layer().compact())
            .try_init()
            .unwrap(),
    }

    match args.commands {
        options::ToposCommand::Tce(cmd) => components::tce::handle_command(cmd).await,
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
