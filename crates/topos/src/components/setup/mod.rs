use self::commands::{SetupCommand, SetupCommands};
use tokio::{signal, spawn};
use tracing::{error, info};

use crate::tracing::setup_tracing;

pub(crate) mod commands;

mod subnet;

pub(crate) async fn handle_command(
    SetupCommand {
        subcommands,
        verbose,
    }: SetupCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(SetupCommands::Subnet(cmd)) => {
            setup_tracing(verbose, None, None)?;

            spawn(async move {
                if cmd.list_releases {
                    info!(
                        "Retrieving release version list from repository: {}",
                        &cmd.repository
                    );
                    if let Err(e) = subnet::installer::list_polygon_edge_releases(cmd).await {
                        error!("Error listing Polygon Edge release versions: {e}");
                        std::process::exit(1);
                    } else {
                        std::process::exit(0);
                    }
                } else {
                    info!(
                        "Starting installation of Polygon Edge binary to target path: {}",
                        &cmd.path.display()
                    );
                    if let Err(e) = subnet::installer::install_polygon_edge(cmd).await {
                        error!("Error installing Polygon Edge: {e}");
                        std::process::exit(1);
                    } else {
                        info!("Polygon Edge installation successful");
                        std::process::exit(0);
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
