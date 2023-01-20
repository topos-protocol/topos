use crate::app_context::AppContext;
use topos_sequencer_certification::CertificationWorker;
use topos_sequencer_subnet_runtime_proxy::{RuntimeProxyConfig, RuntimeProxyWorker};
use topos_sequencer_tce_proxy::{TceProxyConfig, TceProxyWorker};
use topos_sequencer_types::SubnetId;
use tracing::{error, info};
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

    // Launch Tce proxy worker for handling interaction with TCE node
    // TODO implement some retry connection management mechanism
    let (tce_proxy_worker, source_head_certificate_id) = match TceProxyWorker::new(TceProxyConfig {
        subnet_id,
        base_tce_api_url: config.base_tce_api_url.clone(),
    })
    .await
    {
        Ok((tce_proxy_worker, source_head_certificate)) => {
            info!(
                "TCE proxy client is starting for the source subnet {:?} from the head {:?}",
                subnet_id, source_head_certificate
            );
            let source_head_certificate_id = source_head_certificate.map(|cert| cert.id);
            (tce_proxy_worker, source_head_certificate_id)
        }
        Err(e) => {
            error!("Error creating tce proxy worker, error: {e}");
            //TODO Handle retry gracefully
            panic!("Unable to create TCE Proxy");
        }
    };

    // Launch the certification worker for certificate production
    let certification = CertificationWorker::new(subnet_id, source_head_certificate_id)?;

    // Instantiate subnet runtime proxy, handling interaction with subnet node
    let subnet_runtime_proxy = match RuntimeProxyWorker::new(RuntimeProxyConfig {
        subnet_id,
        endpoint: config.subnet_jsonrpc_endpoint.clone(),
        subnet_contract_address: config.subnet_contract_address.clone(),
        keystore_file: config.keystore_file.clone(),
        keystore_password: pass,
    }) {
        Ok(subnet_runtime_proxy) => subnet_runtime_proxy,
        Err(e) => {
            error!("Unable to instantiate runtime proxy, error: {e}");
            //TODO Handle retry connection to subnet node gracefully
            panic!();
        }
    };

    let mut app_context = AppContext::new(
        config,
        certification,
        subnet_runtime_proxy,
        tce_proxy_worker,
    );
    app_context.run().await
}
