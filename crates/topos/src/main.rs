use clap::Parser;

pub(crate) mod components;
pub(crate) mod options;
mod tracing;

#[cfg(feature = "node")]
mod config;

#[cfg(feature = "node")]
mod edge;

use crate::options::ToposCommand;
use tracing_log::LogTracer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    LogTracer::init()?;

    let args = options::Opt::parse();

    match args.commands {
        #[cfg(feature = "setup")]
        ToposCommand::Setup(cmd) => components::setup::handle_command(cmd).await,
        #[cfg(feature = "node")]
        ToposCommand::Node(cmd) => components::node::handle_command(cmd).await,
    }
}
