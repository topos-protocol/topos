use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use opentelemetry::global;
use tokio::{
    signal, spawn,
    sync::{mpsc, oneshot, Mutex},
};
use tonic::transport::Channel;
use topos_core::api::tce::v1::console_service_client::ConsoleServiceClient;
use topos_p2p::PeerId;
use topos_tce::{StorageConfiguration, TceConfiguration};
use tower::Service;
use tracing::{debug, error, info, trace};

use crate::options::input_format::{InputFormat, Parser};

use self::commands::{TceCommand, TceCommands};

pub(crate) mod commands;
pub(crate) mod services;

pub(crate) struct TCEService {
    pub(crate) client: Arc<Mutex<ConsoleServiceClient<Channel>>>,
}

pub(crate) struct PeerList(pub(crate) Option<String>);

impl Parser<PeerList> for InputFormat {
    type Result = Result<Vec<PeerId>, io::Error>;

    fn parse(&self, PeerList(input): PeerList) -> Self::Result {
        let mut input_string = String::new();
        _ = match input {
            Some(path) if Path::new(&path).is_file() => {
                File::open(path)?.read_to_string(&mut input_string)?
            }
            Some(string) => {
                input_string = string;
                0
            }
            None => io::stdin().read_to_string(&mut input_string)?,
        };

        match self {
            InputFormat::Json => Ok(serde_json::from_str::<Vec<PeerId>>(&input_string)?),
            InputFormat::Plain => Ok(input_string
                .trim()
                .split(&[',', '\n'])
                .filter_map(|s| PeerId::from_str(s.trim()).ok())
                .collect()),
        }
    }
}

pub(crate) async fn handle_command(
    TceCommand {
        mut endpoint,
        subcommands,
    }: TceCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(TceCommands::PushPeerList(cmd)) => {
            debug!("Start executing PushPeerList command");
            trace!("Building the gRPC client with {:?}", endpoint);

            let endpoint = endpoint.take().unwrap();
            let client = setup_tce_grpc(&endpoint).await;

            trace!("gRPC client successfully built");

            let mut tce_service = TCEService {
                client: client.clone(),
            };

            debug!("Executing the PushPeerList on the TCE service");
            tce_service.call(cmd).await?;

            Ok(())
        }

        Some(TceCommands::Run(cmd)) => {
            let config = TceConfiguration {
                boot_peers: cmd.parse_boot_peers(),
                local_key_seed: cmd.local_key_seed.map(|s| s.as_bytes().to_vec()),
                jaeger_agent: cmd.jaeger_agent,
                jaeger_service_name: cmd.jaeger_service_name,
                tce_addr: cmd.tce_ext_host,
                tce_local_port: cmd.tce_local_port,
                tce_params: cmd.tce_params,
                api_addr: cmd.api_addr,
                storage: StorageConfiguration::RocksDB(
                    cmd.db_path
                        .as_ref()
                        .and_then(|path| PathBuf::from_str(path).ok()),
                ),
                network_bootstrap_timeout: Duration::from_secs(10),
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
            trace!("Building the gRPC client with {:?}", endpoint);
            let endpoint = endpoint.take().unwrap();
            let client = setup_tce_grpc(&endpoint).await;

            trace!("gRPC client successfully built");

            let mut tce_service = TCEService {
                client: client.clone(),
            };

            debug!("Executing the Status on the TCE service");
            let exit_code = i32::from(!(tce_service.call(status).await?));

            std::process::exit(exit_code);
        }

        None => Ok(()),
    }
}

pub fn print_node_info(config: &TceConfiguration) {
    info!("TCE Node - version: {}", config.version);

    if let StorageConfiguration::RocksDB(Some(ref path)) = config.storage {
        info!("RocksDB at {:?}", path);
    }

    info!("API gRPC endpoint reachable at {}", config.api_addr);
    info!("Jaeger Agent reached at {}", config.jaeger_agent);
    info!("Broadcast Parameters {:?}", config.tce_params);
}

async fn setup_tce_grpc(endpoint: &str) -> Arc<Mutex<ConsoleServiceClient<Channel>>> {
    match ConsoleServiceClient::connect(endpoint.to_string()).await {
        Err(e) => {
            error!("Failure to setup the gRPC API endpoint on {endpoint}: {e}");
            std::process::exit(1);
        }

        Ok(client) => Arc::new(Mutex::new(client)),
    }
}
