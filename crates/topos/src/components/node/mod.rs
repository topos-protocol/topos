use self::commands::{NodeCommand, NodeCommands};
use clap::{CommandFactory, Parser};
use opentelemetry::global;
use tokio::{
    signal, spawn,
    sync::{mpsc, oneshot},
};

use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{config::Config, options::Opt, tracing::setup_tracing};

pub(crate) mod commands;

pub(crate) async fn handle_command(
    NodeCommand {
        subcommands,
        verbose: _,
        home,
    }: NodeCommand,
    args: Opt,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(NodeCommands::Init(_)) => {
            let config = Config::new(args)?;

            println!(
                "Creating the default config file at {}/{}/config.toml",
                home.display(),
                config.node.name
            );
            Ok(())
        }
        Some(NodeCommands::Up(cmd)) => {
            let name = cmd
                .node
                .clone()
                .take()
                .unwrap_or_else(|| "default".to_string());

            let config = Config::load(Opt::parse(), name).node;

            println!(
                "Reading the configuration from {}/{}/config.toml",
                home.display(),
                config.name
            );

            match config.role.as_str() {
                "validator" => {
                    println!("Running a validator!");

                    println!("- Spawning the polygon-edge process");
                    if config.subnet == "topos" {
                        println!("- Spawning the TCE process");
                    }
                }
                "sequencer" => {
                    println!("Running a sequencer!");

                    println!("- Spawning the polygon-edge process");
                    println!("- Spawning the sequencer process");
                    if config.subnet == "topos" {
                        println!("- Spawning the TCE process");
                    }
                }
                _ => {
                    println!("This role is not supported yet");
                }
            }

            Ok(())
        }
        None => Ok(()),
    }
}
