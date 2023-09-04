use self::commands::{SequencerCommand, SequencerCommands};
use std::path::PathBuf;
use std::str::FromStr;
use tokio::{signal, spawn, sync::mpsc};
use tokio_util::sync::CancellationToken;
use topos_sequencer::{self, SequencerConfiguration};
use topos_wallet::SecretManager;
use tracing::{error, info, warn};

use crate::tracing::setup_tracing;

pub(crate) mod commands;

pub(crate) async fn handle_command(
    SequencerCommand {
        verbose,
        subcommands,
    }: SequencerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(SequencerCommands::Run(cmd)) => {
            let keys = SecretManager::from_fs(cmd.subnet_data_dir);
            let config = SequencerConfiguration {
                subnet_id: cmd.subnet_id,
                public_key: None,
                subnet_jsonrpc_endpoint: cmd.subnet_jsonrpc_endpoint,
                subnet_contract_address: cmd.subnet_contract_address,
                tce_grpc_endpoint: cmd.base_tce_api_url,
                signing_key: keys.validator.clone().unwrap(),
                verifier: cmd.verifier,
                db_path: PathBuf::from_str(cmd.db_path.as_str())
                    .expect("Valid path for sequencer db"),
            };

            // Setup instrumentation if both otlp agent and otlp service name are provided as arguments
            setup_tracing(verbose, cmd.otlp_agent, cmd.otlp_service_name)?;

            warn!("DEPRECATED: Please run with `topos node up`");

            print_sequencer_info(&config);
            let shutdown_token = CancellationToken::new();
            let shutdown_trigger = shutdown_token.clone();

            let (shutdown_sender, mut shutdown_receiver) = mpsc::channel(1);

            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("Received ctrl_c, shutting down application...");
                    shutdown_trigger.cancel();

                    // Wait that all sender get dropped
                    let _ = shutdown_receiver.recv().await;

                    info!("Shutdown procedure finished, exiting...");
                }
                result = topos_sequencer::run(config, (shutdown_token, shutdown_sender)) => {
                    if let Err(ref error) = result {
                        error!("Sequencer node terminated {:?}", error);
                        std::process::exit(1);
                    }
                }
            }

            Ok(())
        }
        None => Ok(()),
    }
}

pub fn print_sequencer_info(config: &SequencerConfiguration) {
    info!("Sequencer Node");
    info!("{:?}", config);
}
