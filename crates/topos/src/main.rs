use clap::Parser;

pub(crate) mod components;
pub(crate) mod options;
mod tracing;

use crate::options::ToposCommand;
use tracing_log::LogTracer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    LogTracer::init()?;

    let args = options::Opt::parse();

    match args.commands {
        ToposCommand::Setup(cmd) => components::setup::handle_command(cmd).await,
        ToposCommand::Node(cmd) => components::node::handle_command(cmd).await,
        ToposCommand::Regtest(cmd) => components::regtest::handle_command(cmd).await,
    }
}
