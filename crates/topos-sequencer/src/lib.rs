use crate::app_context::AppContext;
use topos_sequencer_certification::CertificationWorker;
use topos_sequencer_subnet_runtime_proxy::{RuntimeProxyConfig, RuntimeProxyWorker};
use topos_sequencer_tce_proxy::{TceProxyConfig, TceProxyWorker};
use topos_sequencer_types::SubnetId;
use tracing::info;
use tracing_subscriber::{prelude::*, EnvFilter};

mod app_context;

#[derive(Debug)]
pub struct SequencerConfiguration {
    pub subnet_id: String,
    pub subnet_jsonrpc_endpoint: String,
    pub subnet_contract_address: String,
    pub base_tce_api_url: String,
    pub keystore_file: std::path::PathBuf,
    pub keystore_password: Option<String>,
}

pub async fn run(mut config: SequencerConfiguration) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "log-json")]
    let formatting_layer = tracing_subscriber::fmt::layer().json();

    #[cfg(not(feature = "log-json"))]
    let formatting_layer = tracing_subscriber::fmt::layer();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_default())
        .with(formatting_layer)
        .set_default();

    info!("Starting topos-sequencer application");

    let pass = config.keystore_password.take().unwrap_or_else(|| {
        rpassword::prompt_password("Keystore password:").expect("Valid keystore password")
    });

    let subnet_id: SubnetId = hex::decode(&config.subnet_id)?.as_slice().try_into()?;

    // Launch the workers
    let certification = CertificationWorker::new(subnet_id)?;

    let runtime_proxy = RuntimeProxyWorker::new(RuntimeProxyConfig {
        subnet_id,
        endpoint: config.subnet_jsonrpc_endpoint.clone(),
        subnet_contract_address: config.subnet_contract_address.clone(),
        keystore_file: config.keystore_file.clone(),
        keystore_password: pass,
    })?;

    // downstream flow processor worker
    let tce_proxy_worker = TceProxyWorker::new(TceProxyConfig {
        subnet_id,
        base_tce_api_url: config.base_tce_api_url.clone(),
    })
    .await?;

    let mut app_context = AppContext::new(config, certification, runtime_proxy, tce_proxy_worker);
    app_context.run().await
}
