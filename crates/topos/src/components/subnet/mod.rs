use self::commands::{Run, SubnetCommand, SubnetCommands};
use clap::Parser;
use std::process::Stdio;
use tokio::{io::AsyncReadExt, io::BufReader, process, process::Command};
use tokio::{signal, spawn};
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

            let binary_name =
                "polygon-edge-".to_string() + std::env::consts::ARCH + "-" + std::env::consts::OS;
            let polygon_edge_path = cmd.path.to_string() + "/" + &binary_name;

            info!(
                "Running binary {} with arguments:{:?}",
                polygon_edge_path, cmd.polygon_edge_arguments
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
                    Ok(command) => {
                        if let Some(pid) = command.id() {
                            info!("Child process with pid {pid} successfully started");
                        }
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
