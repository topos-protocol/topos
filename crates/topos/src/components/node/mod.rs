use clap::{CommandFactory, Parser};
use figment::error::Kind;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use opentelemetry::global;
use serde_json::{json, Value};
use std::fs;
use std::future::Future;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use std::{
    fs::{create_dir_all, File, OpenOptions},
    io::Write,
    str::FromStr,
};
use tokio::sync::broadcast;
use tokio::{
    signal, spawn,
    sync::{mpsc, oneshot},
};
use tokio_util::sync::CancellationToken;
use topos_p2p::config::NetworkConfig;
use topos_tce_transport::ReliableBroadcastParams;
use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use self::commands::{NodeCommand, NodeCommands};
use crate::config::edge::EdgeConfig;
use crate::config::genesis::Genesis;
use crate::config::sequencer::SequencerConfig;
use crate::config::tce::TceConfig;
use crate::edge::{CommandConfig, BINARY_NAME};
use crate::{
    config::{
        base::BaseConfig, insert_into_toml, load_config, node::NodeConfig, node::NodeRole, Config,
    },
    tracing::setup_tracing,
};
use services::*;
use tokio::process::Command;
use topos_tce::config::{StorageConfiguration, TceConfiguration};
use topos_wallet::SecretManager;

pub(crate) mod commands;
pub mod services;

pub(crate) async fn handle_command(
    NodeCommand {
        subcommands,
        verbose,
        home,
        edge_path,
    }: NodeCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    setup_tracing(verbose, None, None)?;

    match subcommands {
        Some(NodeCommands::Init(cmd)) => {
            let cmd = *cmd;
            let name = cmd.name.as_ref().expect("No name or default was given");

            // Construct path to node config
            // will be $TOPOS_HOME/node/default/ with no given name
            // and $TOPOS_HOME/node/<name>/ with a given name
            let node_path = home.join("node").join(name);

            // If the folders don't exist yet, create it
            create_dir_all(&node_path).expect("failed to create node folder");

            // Check if the config file exists
            let config_path = node_path.join("config.toml");

            if Path::new(&config_path).exists() {
                println!("Config file: {} already exists", config_path.display());
                std::process::exit(1);
            }

            // Generate the configuration as per the role
            let mut config_toml = toml::Table::new();

            // Generate the Edge configuration
            let handle = services::generate_edge_config(edge_path, node_path.clone());

            let node_config = NodeConfig::new(&node_path, Some(cmd));

            // Creating the TOML output
            insert_into_toml(&mut config_toml, node_config);

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

            if let Err(e) = handle.await.unwrap() {
                error!("Failed to init: {e}");
            }

            Ok(())
        }
        Some(NodeCommands::Up(cmd)) => {
            let name = cmd
                .name
                .as_ref()
                .expect("No name or default was given for node");
            let node_path = home.join("node").join(name);
            let config_path = node_path.join("config.toml");

            if !Path::new(&config_path).exists() {
                println!(
                    "Please run 'topos node init --name {name}' to create a config file first for {name}."
                );
                std::process::exit(1);
            }

            // FIXME: Handle properly the `cmd`
            let config = NodeConfig::new(&node_path, None);

            info!(
                "⚙️ Reading the configuration from {}/{}/config.toml",
                home.display(),
                config.base.name
            );

            // Load genesis pointed by the local config
            let genesis = Genesis::new(
                home.join("subnet")
                    .join(config.base.subnet_id.clone())
                    .join("genesis.json"),
            );

            // Get secrets
            let keys = match &config.base.secrets_config {
                Some(secrets_config) => SecretManager::from_aws(secrets_config),
                None => SecretManager::from_fs(node_path.clone()),
            };

            let data_dir = node_path.join(config.edge.clone().unwrap().subnet_data_dir.clone());

            info!(
                "🧢 New joiner: {} for the \"{}\" subnet as {:?}",
                config.base.name, config.base.subnet_id, config.base.role
            );

            let shutdown_token = CancellationToken::new();
            let shutdown_trigger = shutdown_token.clone();
            let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

            let mut processes = FuturesUnordered::new();

            // Edge
            processes.push(services::spawn_edge_process(
                edge_path.join(BINARY_NAME),
                data_dir,
                genesis.path.clone(),
            ));

            // Sequencer
            if matches!(config.base.role, NodeRole::Sequencer) {
                processes.push(services::spawn_sequencer_process(
                    config.sequencer.clone().unwrap(),
                    &keys,
                    (shutdown_token.clone(), shutdown_sender.clone()),
                ));
            }

            // TCE
            if config.base.subnet_id == "topos" {
                processes.push(services::spawn_tce_process(
                    config.tce.clone().unwrap(),
                    keys,
                    genesis,
                    (shutdown_token.clone(), shutdown_sender.clone()),
                ));
            }

            drop(shutdown_sender);

            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("Received ctrl_c, shutting down application...");
                    shutdown(shutdown_trigger, shutdown_receiver).await;
                }
                Some(result) = processes.next() => {
                    info!("Terminate: {result:?}");
                    if let Err(e) = result {
                        error!("Termination: {e}");
                    }
                    shutdown(shutdown_trigger, shutdown_receiver).await;
                    processes.clear();
                }
            };

            Ok(())
        }
        None => Ok(()),
    }
}

pub async fn shutdown(trigger: CancellationToken, mut termination: mpsc::Receiver<()>) {
    trigger.cancel();
    // Wait that all sender get dropped
    info!("Waiting that all components dropped");
    let _ = termination.recv().await;
    info!("Shutdown procedure finished, exiting...");
}
