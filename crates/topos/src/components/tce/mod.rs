use std::{
    fs::File,
    io::{self, Read},
    path::Path,
    str::FromStr,
    sync::Arc,
};

use tokio::{signal, spawn, sync::Mutex};
use tonic::transport::Channel;
use topos_core::api::tce::v1::console_service_client::ConsoleServiceClient;
use topos_p2p::PeerId;
use topos_tce::TceConfiguration;
use tower::Service;
use tracing::{debug, trace};

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

            let client = Arc::new(Mutex::new(
                ConsoleServiceClient::connect(endpoint.take().unwrap())
                    .await
                    .unwrap(),
            ));
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
                db_path: cmd.db_path,
                local_key_seed: cmd.local_key_seed,
                jaeger_agent: cmd.jaeger_agent,
                jaeger_service_name: cmd.jaeger_service_name,
                tce_local_port: cmd.tce_local_port,
                trbp_params: cmd.trbp_params,
                api_addr: cmd.api_addr,
            };

            spawn(async move {
                topos_tce::run(&config).await;
            });

            signal::ctrl_c()
                .await
                .expect("failed to listen for signals");

            Ok(())
        }
        None => Ok(()),
    }
}
