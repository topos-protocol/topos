use self::commands::{SubnetCommand, SubnetCommands};
use std::process::Stdio;
use tokio::{process::Command, signal, spawn};
use tracing::{error, info};

use crate::tracing::setup_tracing;

pub(crate) mod commands;

pub(crate) async fn handle_command(
    SubnetCommand {
        subcommands,
        verbose,
    }: SubnetCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(SubnetCommands::Run(cmd)) => {
            setup_tracing(verbose, None, None)?;

            let binary_name = "polygon-edge".to_string();
            let polygon_edge_path = cmd.path.join(&binary_name);

            info!(
                "Running binary {} with arguments:{:?}",
                polygon_edge_path.display(),
                cmd.polygon_edge_arguments
            );

            spawn(async move {
                // Run Polygon Edge command. Pass all parameters.
                match Command::new(polygon_edge_path)
                    .args(cmd.polygon_edge_arguments)
                    .stderr(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stdin(Stdio::inherit())
                    .spawn()
                {
                    Ok(mut child) => {
                        if let Some(pid) = child.id() {
                            info!("Polygon Edge child process with pid {pid} successfully started");
                        }
                        if let Err(e) = child.wait().await {
                            info!("Polygon Edge child process finished with error: {e}");
                        }
                        std::process::exit(0);
                    }
                    Err(e) => {
                        error!("Error executing Polygon Edge: {e}");
                        std::process::exit(1);
                    }
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
