use self::commands::{SequencerCommand, SequencerCommands};
use tokio::{signal, spawn};
use topos_sequencer::{self, SequencerConfiguration};
use tracing::{error, info};

pub(crate) mod commands;

pub(crate) async fn handle_command(
    SequencerCommand { subcommands }: SequencerCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(SequencerCommands::Run(cmd)) => {
            let config = SequencerConfiguration {
                subnet_id: cmd.subnet_id,
                subnet_jsonrpc_endpoint: cmd.subnet_jsonrpc_endpoint,
                subnet_contract_address: cmd.subnet_contract_address,
                base_tce_api_url: cmd.base_tce_api_url,
                subnet_data_dir_path: cmd.subnet_data_dir,
                verifier: cmd.verifier,
            };

            print_sequencer_info(&config);

            spawn(async move {
                if let Err(error) = topos_sequencer::run(config).await {
                    // TODO: Find a better way
                    panic!("Unable to start the Sequencer node 1 due to : {error:?}");
                }
            });

            signal::ctrl_c()
                .await
                .expect("failed to listen for signals");

            Ok(())
        }
        None => Ok(()),
    }
}

pub fn print_sequencer_info(config: &SequencerConfiguration) {
    info!("Sequencer Node");
    info!("{:?}", config);
}
