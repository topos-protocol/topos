#![allow(unused_imports)]
use std::time::Duration;

use clap::Parser;
#[cfg(feature = "tce")]
use components::tce::commands::{TceCommand, TceCommands};
mod components;
mod options;
mod tracing;

use tracing_log::LogTracer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    LogTracer::init()?;

    let args = options::Opt::parse();
    let verbose = args.verbose;

    match args.commands {
        #[cfg(feature = "tce")]
        options::ToposCommand::Tce(cmd) => components::tce::handle_command(cmd, verbose).await,
        #[cfg(feature = "sequencer")]
        options::ToposCommand::Sequencer(cmd) => {
            components::sequencer::handle_command(cmd, verbose).await
        }
        #[cfg(feature = "checker")]
        options::ToposCommand::Checker(cmd) => {
            components::checker::handle_command(cmd, verbose).await
        }
        #[cfg(feature = "network")]
        options::ToposCommand::Network(cmd) => {
            components::network::handle_command(cmd, verbose).await
        }
    }
}
