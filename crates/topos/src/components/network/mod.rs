use self::commands::{NetworkCommand, NetworkCommands};
use tokio::{signal, spawn};
use topos_certificate_spammer::CertificateSpammerConfig;
use tracing::{error, info};

use crate::tracing::setup_tracing;

pub(crate) mod commands;

pub(crate) async fn handle_command(
    NetworkCommand {
        subcommands,
        verbose,
    }: NetworkCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(NetworkCommands::Spam(cmd)) => {
            let config = CertificateSpammerConfig {
                target_nodes: cmd.target_nodes,
                target_nodes_path: cmd.target_nodes_path,
                local_key_seed: cmd.local_key_seed,
                cert_per_batch: cmd.cert_per_batch,
                nb_subnets: cmd.nb_subnets,
                nb_batches: cmd.nb_batches,
                batch_interval: cmd.batch_interval,
                target_subnets: cmd.target_subnets,
            };

            // Setup instrumentation if both otlp agent and otlp service name
            // are provided as arguments
            // We may want to use instrumentation in e2e tests
            setup_tracing(verbose, cmd.otlp_agent, cmd.otlp_service_name)?;

            spawn(async move {
                if let Err(error) = topos_certificate_spammer::run(config).await {
                    error!("Unable to execute network spam command due to: {error}");
                    std::process::exit(1);
                } else {
                    std::process::exit(0);
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
