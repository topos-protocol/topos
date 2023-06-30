use opentelemetry::global;
use std::str::FromStr;
use std::{
    io::{self, Read},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{
    signal,
    sync::{mpsc, oneshot, Mutex},
};
use tonic::transport::{Channel, Endpoint};
use topos_core::api::grpc::tce::v1::{
    api_service_client::ApiServiceClient, console_service_client::ConsoleServiceClient,
};
use topos_p2p::config::NetworkConfig;
use topos_tce::{StorageConfiguration, TceConfiguration};
use tower::Service;
use tracing::{debug, error, info, warn};

use crate::tracing::setup_tracing;

use self::commands::{TceCommand, TceCommands};

pub(crate) mod commands;
pub(crate) mod parser;
pub(crate) mod services;

pub(crate) struct TCEService {
    pub(crate) console_client: Arc<Mutex<ConsoleServiceClient<Channel>>>,
    pub(crate) _api_client: Arc<Mutex<ApiServiceClient<Channel>>>,
}

impl TCEService {
    pub(crate) fn with_grpc_endpoint(endpoint: &str) -> Self {
        Self {
            console_client: setup_console_tce_grpc(endpoint),
            _api_client: setup_api_tce_grpc(endpoint),
        }
    }
}

pub(crate) async fn handle_command(
    TceCommand {
        verbose,
        mut subcommands,
        ..
    }: TceCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(TceCommands::Run(cmd)) = subcommands.as_mut() {
        // Setup instrumentation if both otlp agent and otlp service name are provided as arguments
        setup_tracing(verbose, cmd.otlp_agent.take(), cmd.otlp_service_name.take())?;
    } else {
        setup_tracing(verbose, None, None)?;
    };

    match subcommands {
        Some(TceCommands::PushCertificate(cmd)) => {
            debug!("Start executing PushCertificate command");
            match services::push_certificate::check_delivery(
                cmd.timeout_broadcast,
                cmd.format,
                cmd.nodes,
                cmd.timeout,
            )
            .await
            .map_err(Box::<dyn std::error::Error>::from)
            {
                Err(_) => {
                    error!("Check failed due to timeout");
                    std::process::exit(1);
                }
                Ok(Err(errors)) => {
                    error!("Check failed due to errors: {:?}", errors);
                    std::process::exit(1);
                }
                _ => {
                    info!("Check passed");
                    Ok(())
                }
            }
        }
        Some(TceCommands::PushPeerList(cmd)) => {
            debug!("Executing the PushPeerList on the TCE service");
            TCEService::with_grpc_endpoint(&cmd.node_args.node)
                .call(cmd)
                .await?;

            Ok(())
        }

        Some(TceCommands::Run(cmd)) => {
            let config = TceConfiguration {
                boot_peers: cmd.parse_boot_peers(),
                local_key_seed: cmd.local_key_seed.map(|s| s.as_bytes().to_vec()),
                tce_addr: cmd.tce_ext_host,
                tce_local_port: cmd.tce_local_port,
                tce_params: cmd.tce_params,
                api_addr: cmd.api_addr,
                graphql_api_addr: cmd.graphql_api_addr,
                storage: StorageConfiguration::RocksDB(
                    cmd.db_path
                        .as_ref()
                        .and_then(|path| PathBuf::from_str(path).ok()),
                ),
                network_bootstrap_timeout: Duration::from_secs(10),
                minimum_cluster_size: cmd
                    .minimum_tce_cluster_size
                    .unwrap_or(NetworkConfig::MINIMUM_CLUSTER_SIZE),
                version: env!("TOPOS_VERSION"),
            };

            print_node_info(&config);

            let (shutdown_sender, shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);

            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("Received ctrl_c, shutting down application...");
                    let (shutdown_finished_sender, shutdown_finished_receiver) = oneshot::channel::<()>();
                    if let Err(e) = shutdown_sender.send(shutdown_finished_sender).await {
                        error!("Error sending shutdown signal to TCE application: {e}");
                    }
                    if let Err(e) = shutdown_finished_receiver.await {
                        error!("Error with shutdown receiver: {e}");
                    }
                    info!("Shutdown procedure finished, exiting...");
                }
                result = topos_tce::run(&config, shutdown_receiver) => {
                    global::shutdown_tracer_provider();
                    if let Err(ref error) = result {
                        error!("TCE node terminated {:?}", error);
                        std::process::exit(1);
                    }
                }

            }

            Ok(())
        }

        Some(TceCommands::Keys(cmd)) => {
            if let Some(slice) = cmd.from_seed {
                println!(
                    "{}",
                    topos_p2p::utils::local_key_pair_from_slice(slice.as_bytes())
                        .public()
                        .to_peer_id()
                )
            };

            Ok(())
        }

        Some(TceCommands::Status(status)) => {
            debug!("Start executing Status command");

            let mut tce_service = TCEService::with_grpc_endpoint(&status.node_args.node);

            debug!("Executing the Status on the TCE service");
            let exit_code = i32::from(!(tce_service.call(status).await?));

            std::process::exit(exit_code);
        }

        None => Ok(()),
    }
}

pub fn print_node_info(config: &TceConfiguration) {
    tracing::warn!("TCE Node - version: {}", config.version);

    if let StorageConfiguration::RocksDB(Some(ref path)) = config.storage {
        info!("RocksDB at {:?}", path);
    }

    warn!("API gRPC endpoint reachable at {}", config.api_addr);
    info!(
        "API GraphQL endpoint reachable at {}",
        config.graphql_api_addr
    );
    warn!("Broadcast Parameters {:?}", config.tce_params);
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

fn setup_api_tce_grpc(endpoint: &str) -> Arc<Mutex<ApiServiceClient<Channel>>> {
    match Endpoint::from_shared(endpoint.to_string()) {
        Ok(endpoint) => Arc::new(Mutex::new(ApiServiceClient::new(endpoint.connect_lazy()))),
        Err(e) => {
            error!("Failure to setup the gRPC API endpoint on {endpoint}: {e}");
            std::process::exit(1);
        }
    }
}
