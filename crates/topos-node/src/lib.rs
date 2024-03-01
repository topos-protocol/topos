//! Temporary lib exposition for backward topos CLI compatibility
use std::{path::PathBuf, process::ExitStatus};

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use opentelemetry::{global, sdk::metrics::controllers::BasicController};
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
use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetrySpanExt;

mod process;

#[allow(clippy::too_many_arguments)]
pub async fn start(
    verbose: u8,
    no_color: bool,
    otlp_agent: Option<String>,
    otlp_service_name: Option<String>,
    no_edge_process: bool,
    node_path: PathBuf,
    home: PathBuf,
    config: NodeConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "⚙️ Reading the configuration from {}/config.toml",
        node_path.display()
    );

    // Load genesis pointed by the local config
    let genesis_file_path = home
        .join("subnet")
        .join(config.base.subnet.clone())
        .join("genesis.json");

    let genesis = match Genesis::new(genesis_file_path.clone()) {
        Ok(genesis) => genesis,
        Err(_) => {
            println!(
                "Could not load genesis.json file on path {} \n Please make sure to have a valid \
                 genesis.json file for your subnet in the {}/subnet/{} folder.",
                genesis_file_path.display(),
                home.display(),
                &config.base.subnet
            );
            std::process::exit(1);
        }
    };

    // Get secrets
    let keys = match &config.base.secrets_config {
        Some(secrets_config) => SecretManager::from_aws(secrets_config),
        None => SecretManager::from_fs(node_path.clone()),
    };

    // Setup instrumentation if both otlp agent and otlp service name
    // are provided as arguments
    let basic_controller = setup_tracing(
        verbose,
        no_color,
        otlp_agent,
        otlp_service_name,
        env!("TOPOS_VERSION"),
    )?;

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
        node_path,
        home,
        genesis,
        shutdown_sender,
        keys,
        shutdown_token,
    );

    let mut sigterm_stream = signal::unix::signal(SignalKind::terminate())?;

    tokio::select! {
        _ = sigterm_stream.recv() => {
            info!("Received SIGTERM, shutting down application...");
            shutdown(basic_controller, shutdown_trigger, shutdown_receiver).await;
        }
        _ = signal::ctrl_c() => {
            info!("Received ctrl_c, shutting down application...");
            shutdown(basic_controller, shutdown_trigger, shutdown_receiver).await;
        }
        Some(result) = processes.next() => {
            shutdown(basic_controller, shutdown_trigger, shutdown_receiver).await;
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

#[allow(clippy::too_many_arguments)]
fn spawn_processes(
    no_edge_process: bool,
    config: NodeConfig,
    node_path: PathBuf,
    edge_path: PathBuf,
    genesis: Genesis,
    shutdown_sender: mpsc::Sender<()>,
    keys: SecretManager,
    shutdown_token: CancellationToken,
) -> FuturesUnordered<JoinHandle<Result<ExitStatus, Errors>>> {
    let processes = FuturesUnordered::new();

    // Edge node
    if no_edge_process {
        info!("Using external edge node, skip running of local edge instance...")
    } else if let Some(edge_config) = config.edge {
        let data_dir = node_path.clone();
        info!(
            "Spawning edge process with genesis file: {}, data directory: {}, additional edge \
             arguments: {:?}",
            genesis.path.display(),
            data_dir.display(),
            edge_config.args
        );
        processes.push(process::spawn_edge_process(
            edge_path.join(BINARY_NAME),
            data_dir,
            genesis.path.clone(),
            edge_config.args,
        ));
    } else {
        error!("Missing edge configuration, could not run edge node!");
        std::process::exit(1);
    }

    // Sequencer
    if matches!(config.base.role, NodeRole::Sequencer) {
        let sequencer_config = config
            .sequencer
            .clone()
            .expect("valid sequencer configuration");
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
        info!("Running topos TCE service...",);
        processes.push(process::spawn_tce_process(
            config.tce.unwrap(),
            keys,
            genesis,
            (shutdown_token.clone(), shutdown_sender.clone()),
        ));
    }

    drop(shutdown_sender);
    processes
}

async fn shutdown(
    basic_controller: Option<BasicController>,
    trigger: CancellationToken,
    mut termination: mpsc::Receiver<()>,
) {
    trigger.cancel();
    // Wait that all sender get dropped
    info!("Waiting that all components dropped");
    let _ = termination.recv().await;
    info!("Shutdown procedure finished, exiting...");
    // Shutdown tracing
    global::shutdown_tracer_provider();
    if let Some(basic_controller) = basic_controller {
        if let Err(e) = basic_controller.stop(&tracing::Span::current().context()) {
            error!("Error stopping tracing: {e}");
        }
    }
}
