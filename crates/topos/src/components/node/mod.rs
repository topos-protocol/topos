use std::{
    fs::{create_dir_all, remove_dir_all, OpenOptions},
    io::Write,
};
use std::{path::Path, sync::Arc};
use tokio::sync::Mutex;
use tonic::transport::{Channel, Endpoint};
use topos_telemetry::tracing::setup_tracing;
use tower::Service;
use tracing::error;

use self::commands::{NodeCommand, NodeCommands};
use topos_config::node::NodeConfig;
use topos_config::{edge::command::BINARY_NAME, Config};
use topos_core::api::grpc::tce::v1::console_service_client::ConsoleServiceClient;

pub(crate) mod commands;
pub(crate) mod services;

pub(crate) struct NodeService {
    pub(crate) console_client: Arc<Mutex<ConsoleServiceClient<Channel>>>,
}

impl NodeService {
    pub(crate) fn with_grpc_endpoint(endpoint: &str) -> Self {
        Self {
            console_client: setup_console_tce_grpc(endpoint),
        }
    }
}

pub(crate) async fn handle_command(
    NodeCommand {
        subcommands,
        verbose,
        no_color,
        home,
        edge_path,
    }: NodeCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(NodeCommands::Init(cmd)) => {
            let cmd = *cmd;
            let name = cmd.name.clone().expect("No name or default was given");

            _ = setup_tracing(verbose, no_color, None, None, env!("TOPOS_VERSION"));
            // Construct path to node config
            // will be $TOPOS_HOME/node/default/ with no given name
            // and $TOPOS_HOME/node/<name>/ with a given name
            let node_path = home.join("node").join(&name);

            // If the folders don't exist yet, create it
            create_dir_all(&node_path).expect("failed to create node folder");

            // Check if the config file exists
            let config_path = node_path.join("config.toml");

            if Path::new(&config_path).exists() {
                println!("Config file: {} already exists", config_path.display());
                std::process::exit(1);
            }

            if cmd.no_edge_process {
                println!("Init the node without polygon-edge process...");
            } else {
                // Generate the Edge configuration
                match topos_config::edge::generate_edge_config(
                    edge_path.join(BINARY_NAME),
                    node_path.clone(),
                )
                .await
                {
                    Ok(Ok(status)) => {
                        if let Some(0) = status.code() {
                            println!("Edge configuration successfully generated");
                        } else {
                            println!(
                                "Edge configuration generation terminated with error status: {:?}",
                                status
                            );
                            remove_dir_all(node_path).expect("failed to remove config folder");
                            std::process::exit(1);
                        }
                    }
                    Ok(Err(e)) => {
                        println!("Failed to generate edge config with error {e}");
                        remove_dir_all(node_path).expect("failed to remove config folder");
                        std::process::exit(1);
                    }
                    Err(_) => {
                        println!("Failed to generate edge config");
                        remove_dir_all(node_path).expect("failed to remove config folder");
                        std::process::exit(1);
                    }
                }
            }

            let node_config = NodeConfig::create(&home, &name, Some(cmd));

            // Creating the TOML output
            let config_toml = match node_config.to_toml() {
                Ok(config) => config,
                Err(error) => {
                    println!("Failed to generate TOML config: {error}");
                    remove_dir_all(node_path).expect("failed to remove config folder");
                    std::process::exit(1);
                }
            };

            let config_path = node_path.join("config.toml");
            let mut node_config_file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(config_path)
                .expect("failed to create default node file");

            node_config_file
                .write_all(config_toml.to_string().as_bytes())
                .expect("failed to write to default node file");

            println!(
                "Created node config file at {}/config.toml",
                node_path.display()
            );

            Ok(())
        }
        Some(NodeCommands::Up(cmd)) => {
            let cmd_cloned = cmd.clone();
            let command = *cmd;

            let name = cmd_cloned
                .name
                .as_ref()
                .expect("No name or default was given for node");

            let config = NodeConfig::try_from(&home, name, Some(command))?;

            topos_node::start(
                verbose,
                no_color,
                cmd_cloned.otlp_agent,
                cmd_cloned.otlp_service_name,
                cmd_cloned.no_edge_process,
                config,
            )
            .await?;

            Ok(())
        }
        Some(NodeCommands::Status(status)) => {
            let mut node_service = NodeService::with_grpc_endpoint(&status.node_args.node);
            let exit_code = i32::from(!(node_service.call(status).await?));
            std::process::exit(exit_code);
        }
        None => Ok(()),
    }
}

fn setup_console_tce_grpc(endpoint: &str) -> Arc<Mutex<ConsoleServiceClient<Channel>>> {
    match Endpoint::from_shared(endpoint.to_string()) {
        Ok(endpoint) => Arc::new(Mutex::new(ConsoleServiceClient::new(
            endpoint.connect_lazy(),
        ))),
        Err(e) => {
            error!("Failure to setup the gRPC API endpoint on {endpoint}: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn shutdown(
    basic_controller: Option<BasicController>,
    trigger: CancellationToken,
    mut termination: mpsc::Receiver<()>,
) {
    trigger.cancel();
    info!("Waiting for all components to stop");
    // let _ = termination.recv().await;
    match termination.recv().await {
        Some(x) => info!("All good in the hood? {:?}", x),
        None => tracing::warn!("Odd, got a None. What does that mean?"),
    }
    info!("Shutdown complete, exiting.");

    // Shutdown tracing
    global::shutdown_tracer_provider();
    if let Some(basic_controller) = basic_controller {
        if let Err(e) = basic_controller.stop(&tracing::Span::current().context()) {
            error!("Error stopping tracing: {e}");
        }
    }
}
