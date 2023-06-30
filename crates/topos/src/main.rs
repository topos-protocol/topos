#![allow(unused_imports)]
use std::time::Duration;

use clap::CommandFactory;
use clap::{Args, Parser, Subcommand};

pub(crate) mod components;
mod config;
pub(crate) mod options;
mod tracing;

#[cfg(feature = "tce")]
use components::tce::commands::{TceCommand, TceCommands};

use crate::options::ToposCommand;
use tracing_log::LogTracer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    LogTracer::init()?;

    let args = options::Opt::parse();

    match args.commands {
        #[cfg(feature = "tce")]
        ToposCommand::Tce(cmd) => components::tce::handle_command(cmd).await,
        #[cfg(feature = "sequencer")]
        ToposCommand::Sequencer(cmd) => components::sequencer::handle_command(cmd).await,
        #[cfg(feature = "network")]
        ToposCommand::Network(cmd) => components::network::handle_command(cmd).await,
        #[cfg(feature = "setup")]
        ToposCommand::Setup(cmd) => components::setup::handle_command(cmd).await,
        #[cfg(feature = "subnet")]
        ToposCommand::Subnet(cmd) => components::subnet::handle_command(cmd).await,
        #[cfg(feature = "node")]
        ToposCommand::Node(cmd) => components::node::handle_command(cmd).await,
        ToposCommand::Doctor => components::doctor::handle_doctor().await,
    }
}
