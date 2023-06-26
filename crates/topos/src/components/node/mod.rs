use clap::{CommandFactory, Parser};
use opentelemetry::global;
use tokio::{
    signal, spawn,
    sync::{mpsc, oneshot},
};

use figment::error::Kind;
use std::path::Path;
use std::{
    fs::{create_dir_all, File, OpenOptions},
    io::Write,
    str::FromStr,
};

use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use self::commands::{NodeCommand, NodeCommands};
use crate::{
    config::{
        base::BaseConfig, insert_into_toml, load_config, node::NodeConfig,
        sequencer::SequencerConfig, tce::TceConfig, Config,
    },
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
            let name = cmd.name.expect("No name or default was given");

            // Construct path to node config
            // will be $TOPOS_HOME/node/default/ with no given name
            // and $TOPOS_HOME/node/<name>/ with a given name
            let node_path = home.join("node").join(name);

            // If the folders don't exist yet, create it
            create_dir_all(&node_path).expect("failed to create node folder");

            // Check if the config file exists
            let config_path = home.join("config.toml");

            if Path::new(&config_path).exists() {
                println!("Config file: {} already exists", config_path.display());
                std::process::exit(1);
            }

            let base_config = load_config::<BaseConfig>(&node_path);
            let tce_config = load_config::<TceConfig>(&node_path);
            let sequencer_config = load_config::<SequencerConfig>(&node_path);

            // Creating the TOML output
            let mut config_toml = toml::Table::new();
            insert_into_toml(&mut config_toml, base_config);
            insert_into_toml(&mut config_toml, tce_config);
            insert_into_toml(&mut config_toml, sequencer_config);

            let config_path = node_path.join("config.toml");
            let mut node_config_file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(config_path)
                .expect("failed to create default node file");

            node_config_file
                .write_all(toml::to_string(&config_toml).unwrap().as_bytes())
                .expect("failed to write to default node file");

            println!(
                "Created node config file at {}/config.toml",
                node_path.display()
            );

            Ok(())
        }
        Some(NodeCommands::Up(cmd)) => {
            let node = cmd.node.expect("No name or default was given for node");
            let node_path = home.join("node").join(node.clone());
            let config_path = home.join("node").join(node.clone()).join("config.toml");

            if !Path::new(&config_path).exists() {
                println!("Please run 'topos init -n {node}' to create a config file first.");
                std::process::exit(1);
            }

            let config = NodeConfig::load(&node_path, None)?;

            println!(
                "Reading the configuration from {}/{}/config.toml",
                home.display(),
                config.base.name
            );

            match config.base.role.as_str() {
                "validator" => {
                    println!("Running a validator!");

                    println!("- Spawning the polygon-edge process");
                    if config.base.subnet == "topos" {
                        println!("- Spawning the TCE process");
                    }
                }
                "sequencer" => {
                    println!("Running a sequencer!");

                    println!("- Spawning the polygon-edge process");
                    println!("- Spawning the sequencer process");
                    if config.base.subnet == "topos" {
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
