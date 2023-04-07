use self::commands::{NetworkCommand, NetworkCommands};
use tokio::{signal, spawn};
use topos_certificate_spammer::CertificateSpammerConfiguration;
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
            let config = CertificateSpammerConfiguration {
                target_node: cmd.target_node,
                target_nodes_path: cmd.target_nodes_path,
                cert_per_batch: cmd.cert_per_batch,
                batch_time_interval: cmd.batch_time_interval,
                nb_subnets: cmd.nb_subnets,
                byzantine_threshold: cmd.byzantine_threshold,
                node_per_cert: cmd.node_per_cert,
            };

            // Setup instrumentation if both otlp agent and otlp service name
            // are provided as arguments
            // We may want to use instrumentation in e2e tests
            setup_tracing(verbose, cmd.otlp_agent, cmd.otlp_service_name)?;

            spawn(async move {
                if let Err(error) = topos_certificate_spammer::run(config).await {
                    panic!("Unable to execute network spam command due to : {error:?}");
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
