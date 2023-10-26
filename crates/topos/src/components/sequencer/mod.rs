use self::commands::{SequencerCommand, SequencerCommands};
use tokio::{signal, sync::mpsc};
use tokio_util::sync::CancellationToken;
use topos_sequencer::{self, SequencerConfiguration};
use topos_wallet::SecretManager;
use tracing::{error, info, warn};

pub(crate) mod commands;

pub(crate) async fn handle_command(
    SequencerCommand { subcommands }: SequencerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(SequencerCommands::Run(cmd)) => {
            let keys = SecretManager::from_fs(cmd.subnet_data_dir);
            let config = SequencerConfiguration {
                subnet_id: cmd.subnet_id,
                public_key: None,
                subnet_jsonrpc_http: cmd.subnet_jsonrpc_http,
                subnet_jsonrpc_ws: cmd.subnet_jsonrpc_ws,
                subnet_contract_address: cmd.subnet_contract_address,
                tce_grpc_endpoint: cmd.base_tce_api_url,
                signing_key: keys.validator.clone().unwrap(),
                verifier: cmd.verifier,
                start_block: cmd.start_block,
            };

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
