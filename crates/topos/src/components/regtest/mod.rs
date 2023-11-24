use self::commands::{RegtestCommand, RegtestCommands};

use opentelemetry::global;
use tokio::{
    spawn,
    sync::{mpsc, oneshot},
};
use topos_certificate_spammer::CertificateSpammerConfig;
use tracing::{debug, error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::tracing::setup_tracing;

pub(crate) mod commands;
pub(crate) mod services;

pub(crate) async fn handle_command(
    RegtestCommand {
        verbose,
        subcommands,
    }: RegtestCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(RegtestCommands::PushCertificate(cmd)) => {
            _ = setup_tracing(verbose, None, None)?;
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
                Err(e) => {
                    error!("Check failed: {:?}", e);
                    std::process::exit(1);
                }
                _ => {
                    info!("Check passed");
                    Ok(())
                }
            }
        }
        Some(RegtestCommands::Spam(cmd)) => {
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
            let basic_controller = setup_tracing(verbose, cmd.otlp_agent, cmd.otlp_service_name)?;

            let (shutdown_sender, shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);
            let mut runtime = spawn(topos_certificate_spammer::run(config, shutdown_receiver));

            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        info!("Received ctrl_c, shutting down application...");

                        let (shutdown_finished_sender, shutdown_finished_receiver) = oneshot::channel::<()>();
                        if let Err(e) = shutdown_sender.send(shutdown_finished_sender).await {
                            error!("Error sending shutdown signal to Spammer application: {e}");
                        }
                        if let Err(e) = shutdown_finished_receiver.await {
                            error!("Error with shutdown receiver: {e}");
                        }
                        info!("Shutdown procedure finished, exiting...");
                    }
                    result = &mut runtime =>{
                        global::shutdown_tracer_provider();
                        if let Some(basic_controller) = basic_controller {
                            if let Err(e) = basic_controller.stop(&tracing::Span::current().context()) {
                                error!("Error stopping tracing: {e}");
                            }
                        }

                        if let Err(ref error) = result {
                            error!("Unable to execute network spam command due to: {error}");
                            std::process::exit(1);
                        }
                        break;
                    }
                }
            }

            Ok(())
        }
        None => Ok(()),
    }
}
