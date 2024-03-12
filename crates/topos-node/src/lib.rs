//! Temporary lib exposition for backward topos CLI compatibility
use std::process::ExitStatus;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use opentelemetry::global;
use process::Errors;
use tokio::{
    signal::{self, unix::SignalKind},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use topos_config::{
    edge::command::BINARY_NAME,
    genesis::Genesis,
    node::{NodeConfig, NodeRole},
};
use topos_telemetry::tracing::setup_tracing;
use topos_wallet::SecretManager;
use tracing::{debug, error, info};
use tracing_subscriber::util::TryInitError;

mod process;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    GenesisFile(#[from] topos_config::genesis::Error),

    #[error("Unable to setup tracing logger: {0}")]
    Tracing(#[from] TryInitError),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(
        "The role in the config file expect to have a sequencer config defined, none was found"
    )]
    MissingSequencerConfig,

    #[error("An Edge config was expected to be found in the config file")]
    MissingEdgeConfig,

    #[error("A TCE config was expected to be found in the config file")]
    MissingTCEConfig,
}

pub async fn start(
    verbose: u8,
    no_color: bool,
    otlp_agent: Option<String>,
    otlp_service_name: Option<String>,
    no_edge_process: bool,
    config: NodeConfig,
) -> Result<(), Error> {
    // Setup instrumentation if both otlp agent and otlp service name
    // are provided as arguments
    setup_tracing(
        verbose,
        no_color,
        otlp_agent,
        otlp_service_name,
        env!("TOPOS_VERSION"),
    )?;

    info!(
        "⚙️ Read the configuration from {}/config.toml",
        config.node_path.display()
    );

    debug!("TceConfig: {:?}", config);

    let config_ref = &config;
    let genesis: Genesis = config_ref.try_into().map_err(|error| {
        info!(
            "Could not load genesis.json file on path {} \n Please make sure to have a valid \
             genesis.json file for your subnet in the {}/subnet/{} folder.",
            config.edge_path.display(),
            config.home_path.display(),
            &config.base.subnet
        );

        error
    })?;

    // Get secrets
    let keys: SecretManager = config_ref.into();

    info!(
        "🧢 New joiner: {} for the \"{}\" subnet as {:?}",
        config.base.name, config.base.subnet, config.base.role
    );

    let shutdown_token = CancellationToken::new();
    let shutdown_trigger = shutdown_token.clone();

    let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

    let mut processes = spawn_processes(
        no_edge_process,
        config,
        genesis,
        shutdown_sender,
        keys,
        shutdown_token,
    )?;

    let mut sigterm_stream = signal::unix::signal(SignalKind::terminate())?;

    tokio::select! {
        _ = sigterm_stream.recv() => {
            info!("Received SIGTERM, shutting down application...");
            shutdown(shutdown_trigger, shutdown_receiver).await;
        }
        _ = signal::ctrl_c() => {
            info!("Received ctrl_c, shutting down application...");
            shutdown( shutdown_trigger, shutdown_receiver).await;
        }
        Some(result) = processes.next() => {
            shutdown(shutdown_trigger, shutdown_receiver).await;
            processes.clear();
            match result {
                Ok(Ok(status)) => {
                    if let Some(0) = status.code() {
                        info!("Terminating with success error code");
                    } else {
                        info!("Terminating with error status: {:?}", status);
                        std::process::exit(1);
                    }
                }
                Ok(Err(e)) => {
                    error!("Terminating with error: {e}");
                    std::process::exit(1);
                }
                Err(e) => {
                    error!("Terminating with error: {e}");
                    std::process::exit(1);
                }
            }
        }
    };

    Ok(())
}

fn spawn_processes(
    no_edge_process: bool,
    mut config: NodeConfig,
    genesis: Genesis,
    shutdown_sender: mpsc::Sender<()>,
    keys: SecretManager,
    shutdown_token: CancellationToken,
) -> Result<FuturesUnordered<JoinHandle<Result<ExitStatus, Errors>>>, Error> {
    let processes = FuturesUnordered::new();

    // Edge node
    if no_edge_process {
        info!("Using external edge node, skip running of local edge instance...")
    } else {
        let edge_config = config.edge.take().ok_or(Error::MissingEdgeConfig)?;

        let data_dir = config.node_path.clone();

        info!(
            "Spawning edge process with genesis file: {}, data directory: {}, additional edge \
             arguments: {:?}",
            config.edge_path.display(),
            data_dir.display(),
            edge_config.args
        );

        processes.push(process::spawn_edge_process(
            config.home_path.join(BINARY_NAME),
            data_dir,
            config.edge_path.clone(),
            edge_config.args,
        ));
    }

    // Sequencer
    if matches!(config.base.role, NodeRole::Sequencer) {
        let sequencer_config = config
            .sequencer
            .take()
            .ok_or(Error::MissingSequencerConfig)?;

        info!(
            "Running sequencer with configuration {:?}",
            sequencer_config
        );
        processes.push(process::spawn_sequencer_process(
            sequencer_config,
            &keys,
            (shutdown_token.clone(), shutdown_sender.clone()),
        ));
    }

    // TCE
    if config.base.subnet == "topos" {
        let tce_config = config.tce.ok_or(Error::MissingTCEConfig)?;
        info!("Running topos TCE service...",);

        processes.push(process::spawn_tce_process(
            tce_config,
            keys,
            genesis,
            (shutdown_token.clone(), shutdown_sender.clone()),
        ));
    }

    drop(shutdown_sender);
    Ok(processes)
}

async fn shutdown(trigger: CancellationToken, mut termination: mpsc::Receiver<()>) {
    trigger.cancel();
    // Wait that all sender get dropped
    info!("Waiting that all components dropped");
    let _ = termination.recv().await;
    info!("Shutdown procedure finished, exiting...");
    // Shutdown tracing
    global::shutdown_tracer_provider();
}
