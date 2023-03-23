use self::commands::{
    assert_delivery::AssertDelivery, CheckerCommand, CheckerCommands, CheckerTceCommand,
    CheckerTceCommands,
};
use futures::{future::join_all, StreamExt, TryFutureExt};
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
    time::Duration,
};
use topos_core::{
    api::{
        shared::v1::checkpoints::TargetCheckpoint,
        tce::v1::{
            api_service_client::ApiServiceClient,
            console_service_client::ConsoleServiceClient,
            watch_certificates_request::{Command, OpenStream},
            watch_certificates_response::{CertificatePushed, Event},
            StatusRequest, SubmitCertificateRequest, WatchCertificatesRequest,
        },
    },
    uci::Certificate,
};
use tower::Service;
use tracing::{error, info, trace};

use crate::options::input_format::{InputFormat, Parser};
use crate::{components::checker::parser::NodeList, tracing::setup_tracing};

pub(crate) mod commands;
pub(crate) mod parser;
pub(crate) mod services;

pub(crate) async fn handle_command(
    CheckerCommand { subcommands }: CheckerCommand,
    verbose: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    match subcommands {
        Some(CheckerCommands::Tce(CheckerTceCommand {
            subcommands:
                Some(CheckerTceCommands::AssertDelivery(AssertDelivery {
                    timeout_broadcast,
                    format,
                    nodes: peers,
                    timeout,
                })),
        })) => {
            setup_tracing(verbose, None, None)?;

            match services::check_delivery(timeout_broadcast, format, peers, timeout)
                .await
                .map_err(Box::<dyn std::error::Error>::from)
            {
                Err(_) => {
                    error!("Check failed due to timeout");
                    std::process::exit(1);
                }
                Ok(Err(errors)) => {
                    error!("Check failed due to errors: {:?}", errors);
                    std::process::exit(1);
                }
                _ => {
                    info!("Check passed");
                    Ok(())
                }
            }
        }
        _ => todo!(),
    }
}
