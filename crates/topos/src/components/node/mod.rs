use clap::{CommandFactory, Parser};
use figment::error::Kind;
use futures::stream::FuturesUnordered;
use opentelemetry::global;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use std::{
    fs::{create_dir_all, File, OpenOptions},
    io::Write,
    str::FromStr,
};
use tokio::{
    signal, spawn,
    sync::{mpsc, oneshot},
};
use topos_p2p::config::NetworkConfig;
use topos_tce_transport::ReliableBroadcastParams;

use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use self::commands::{NodeCommand, NodeCommands};
use crate::config::edge::EdgeConfig;
use crate::config::sequencer::SequencerConfig;
use crate::config::tce::TceConfig;
use crate::edge::{CommandConfig, BINARY_NAME};
use tokio::process::Command;

use crate::{
    config::{
        base::BaseConfig, insert_into_toml, load_config, node::NodeConfig, node::NodeRole, Config,
    },
    tracing::setup_tracing,
};
use std::path::PathBuf;
use topos_tce::config::{StorageConfiguration, TceConfiguration};

pub(crate) mod commands;
pub mod services;

pub fn generate_edge_config(
    edge_path: PathBuf,
    config_path: PathBuf,
) -> impl Future<Output = Result<ExitStatus, std::io::Error>> {
    // Create the Polygon Edge config
    let polygon_edge_path = edge_path.join(BINARY_NAME);

    CommandConfig::new(polygon_edge_path)
        .init(&config_path)
        .spawn()
}

async fn spawn_validator(edge_path: PathBuf, subnet_path: PathBuf, config: NodeConfig) {
    println!(
        "🧢  {} joining the {} subnet as {:?}",
        config.base.name, config.base.subnet, config.base.role
    );

    let subnet_genesis = subnet_path.join("genesis.json");

    let edge_component = spawn_edge_process(edge_path, config.edge.subnet_data_dir, subnet_genesis);

    let tce_config = TceConfiguration {
        boot_peers: config.tce.parse_boot_peers(),
        local_key_seed: config.tce.local_key_seed.map(|s| s.as_bytes().to_vec()),
        tce_addr: config.tce.tce_ext_host,
        tce_local_port: config.tce.tce_local_port,
        tce_params: ReliableBroadcastParams {
            echo_threshold: config.tce.echo_threshold,
            ready_threshold: config.tce.ready_threshold,
            delivery_threshold: config.tce.delivery_threshold,
        },
        api_addr: config.tce.api_addr,
        graphql_api_addr: config.tce.graphql_api_addr,
        metrics_api_addr: config.tce.metrics_api_addr,
        storage: StorageConfiguration::RocksDB(PathBuf::from_str(&config.tce.db_path).ok()),
        network_bootstrap_timeout: Duration::from_secs(10),
        minimum_cluster_size: config
            .tce
            .minimum_tce_cluster_size
            .unwrap_or(NetworkConfig::MINIMUM_CLUSTER_SIZE),
        version: env!("TOPOS_VERSION"),
    };

    spawn(edge_component);

    // TCE component if we are Topos
    if config.base.subnet == "topos" {
        let _ = topos_tce::spawn_tce_component(&tce_config).await;
    }
}

#[allow(dead_code)]
pub fn spawn_edge_process(
    edge_path: PathBuf,
    data_dir: PathBuf,
    genesis_path: PathBuf,
) -> impl Future<Output = ()> {
    // Create the Polygon Edge config
    let polygon_edge_path = edge_path.join(BINARY_NAME);

    CommandConfig::new(polygon_edge_path)
        .server(&data_dir, &genesis_path)
        .spawn()
}

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
            let handle = spawn(generate_edge_config(edge_path, node_path.clone()));

            let base_config = load_config::<BaseConfig>(&node_path, Some(cmd));
            let tce_config = load_config::<TceConfig>(&node_path, None);
            let sequencer_config = load_config::<SequencerConfig>(&node_path, None);
            let edge_config = load_config::<EdgeConfig>(&node_path, None);

            // Creating the TOML output
            insert_into_toml(&mut config_toml, base_config);
            insert_into_toml(&mut config_toml, tce_config);
            insert_into_toml(&mut config_toml, sequencer_config);
            insert_into_toml(&mut config_toml, edge_config);

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

            let _ = handle.await.unwrap();

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

            let config = load_config::<NodeConfig>(&node_path, Some(*cmd));

            info!(
                "⚙️ Reading the configuration from {}/{}/config.toml",
                home.display(),
                config.base.name
            );

            let genesis_path = home
                .join("subnet")
                .join(config.base.subnet_id.clone())
                .join("genesis.json");

            match config.base.role {
                NodeRole::Validator => {
                    spawn_validator(edge_path, subnet_path, config).await;
                }
                NodeRole::Sequencer => {
                    println!("Running a sequencer!");

                    println!("- Spawning the polygon-edge process");
                    println!("- Spawning the sequencer process");
                    if config.base.subnet == "topos" {
                        println!("- Spawning the TCE process");
                    }
                }
                NodeRole::FullNode => {
                    println!("Running a full node!");
                }
            }

            Ok(())
        }
        None => Ok(()),
    }
}
