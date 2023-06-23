use self::commands::{NodeCommand, NodeCommands};
use clap::{CommandFactory, Parser};
use opentelemetry::global;
use tokio::{
    signal, spawn,
    sync::{mpsc, oneshot},
};

use std::{
    fs::{create_dir_all, File, OpenOptions},
    io::Write,
    str::FromStr,
};
use std::path::Path;
use figment::error::Kind;

use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::{
    config::{Config, SequencerConfig, TceConfig},
    tracing::setup_tracing,
};

pub(crate) mod commands;

pub(crate) async fn handle_command(
    NodeCommand {
        subcommands,
        verbose: _,
        home,
    }: NodeCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(NodeCommands::Init(cmd)) => {
            // Construct path to node config
            // will be $TOPOS_HOME/node/default/ with no given name
            // and $TOPOS_HOME/node/<name>/ with a given name
            let node_path = home
                .join("node")
                .join(cmd.name);

            // If the folders don't exist yet, create it
            create_dir_all(&node_path).expect("failed to create home folder");

            // Check if the config file exists
            let config_path = home.join("config.toml");

            if Path::new(&config_path).exists() {
                println!("Config file: {config_path} already exists");
                std::process::exit(1);
            }

            // TODO: Wipe the file if it exists, need to open
            let _config_file = File::create(config_path).expect("failed to create config file");

            let node_path = home.join("node");
            create_dir_all(&node_path).expect("failed to create node folder");

            let default_node_path = node_path.join("default.toml");

            // Handle config missing key here
            let tce_config = match TceConfig::load(&node_path, None) {
                Ok(config) => config,
                Err(figment::Error {
                        kind: Kind::MissingField(name),
                        ..
                    }) => {
                    println!("Missing field: {}", name);
                    std::process::exit(1);
                }
                _ => {
                    println!("Failed to load config");
                    std::process::exit(1);
                }
            };

            // Handle config missing key here
            let sequencer_config = match SequencerConfig::load(&node_path, None) {
                Ok(config) => config,
                Err(figment::Error {
                        kind: Kind::MissingField(name),
                        ..
                    }) => {
                    println!("Missing field: {}", name);
                    std::process::exit(1);
                }
                _ => {
                    println!("Failed to load config");
                    std::process::exit(1);
                }
            };
            // Creating the TOML output
            let mut toml_default = toml::Table::new();

            toml_default.insert(
                tce_config.profile(),
                toml::Value::Table(
                    tce_config
                        .to_toml()
                        .expect("failed to convert tce config to toml"),
                ),
            );

            toml_default.insert(
                sequencer_config.profile(),
                toml::Value::Table(
                    sequencer_config
                        .to_toml()
                        .expect("failed to convert sequencer config to toml"),
                ),
            );

            println!(
                "toml_default: {:?}",
                toml::to_string(&toml_default).unwrap().as_bytes()
            );
            println!("Default node path: {:?}", default_node_path);
            let mut default_node_file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(default_node_path)
                .expect("failed to create default node file");

            _ = default_node_file
                .write_all(toml::to_string(&toml_default).unwrap().as_bytes())
                .expect("failed to write to default node file");

            Ok(())
        }
        Some(NodeCommands::Up(cmd)) => {

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
