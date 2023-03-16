use crate::app_context::AppContext;
use std::io::ErrorKind;
use std::io::ErrorKind::InvalidInput;
use topos_sequencer_certification::CertificationWorker;
use topos_sequencer_subnet_runtime_proxy::{SubnetRuntimeProxyConfig, SubnetRuntimeProxyWorker};
use topos_sequencer_tce_proxy::{TceProxyConfig, TceProxyWorker};
use topos_sequencer_types::SubnetId;
use tracing::{debug, error, info};

mod app_context;

#[derive(Debug)]
pub struct SequencerConfiguration {
    pub subnet_id: Option<String>,
    pub subnet_jsonrpc_endpoint: String,
    pub subnet_contract_address: String,
    pub base_tce_api_url: String,
    pub subnet_data_dir_path: std::path::PathBuf,
    pub verifier: u32,
}

pub async fn run(config: SequencerConfiguration) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Starting topos-sequencer application");

    let subnet_data_dir_path = config
        .subnet_data_dir_path
        .to_str()
        .ok_or(std::io::Error::new(
            ErrorKind::InvalidInput,
            "Invalid subnet data dir path",
        ))?
        .to_string();

    let key_file_path = topos_crypto::keystore::get_keystore_path(subnet_data_dir_path.as_str());
    //TODO handle this key in more secure way (e.g. use SafeSecretKey)
    let signing_key: Vec<u8> =
        match topos_crypto::keystore::read_private_key_from_file(&key_file_path, None) {
            Ok(key) => key,
            Err(e) => {
                error!("Unable to get ethereum private key from keystore: {}", e);

                return Err(Box::new(std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "Unable to read private key from keystore file",
                )));
            }
        };

    // If subnetID is specified as command line argument, use it
    let subnet_id: SubnetId = if let Some(subnet_id) = &config.subnet_id {
        if &subnet_id[0..2] != "0x" {
            return Err(Box::new(std::io::Error::new(
                InvalidInput,
                "Subnet id must start with `0x`",
            )));
        }
        hex::decode(&subnet_id[2..])?.as_slice().try_into()?
    }
    // Get subnet id from the subnet node if not provided via the command line argument
    // It will retry using backoff algorithm, but if it fails (default max backoff elapsed time is 15 min) we can not proceed
    else {
        match SubnetRuntimeProxyWorker::get_subnet_id(
            config.subnet_jsonrpc_endpoint.as_str(),
            config.subnet_contract_address.as_str(),
        )
        .await
        {
            Ok(subnet_id) => {
                info!("Retrieved subnet id from the subnet node {subnet_id}");
                subnet_id
            }
            Err(e) => {
                panic!("Unable to get subnet id from the subnet {e}");
            }
        }
    };

    // Instantiate subnet runtime proxy, handling interaction with subnet node
    let subnet_runtime_proxy = match SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id,
            endpoint: config.subnet_jsonrpc_endpoint.clone(),
            subnet_contract_address: config.subnet_contract_address.clone(),
        },
        signing_key.clone(),
    )
    .await
    {
        Ok(subnet_runtime_proxy) => subnet_runtime_proxy,
        Err(e) => {
            panic!("Unable to instantiate runtime proxy, error: {e}");
        }
    };

    // Get subnet checkpoints from subnet to pass them to the TCE node
    // It will retry using backoff algorithm, but if it fails (default max backoff elapsed time is 15 min) we can not proceed
    let target_subnet_stream_positions = match subnet_runtime_proxy.get_checkpoints().await {
        Ok(checkpoints) => checkpoints,
        Err(e) => {
            panic!("Unable to get checkpoints from the subnet {e}");
        }
    };

    // Launch Tce proxy worker for handling interaction with TCE node
    // For initialization it will retry using backoff algorithm, but if it fails (default max backoff elapsed time is 15 min) we can not proceed
    // Once it is initialized, TCE proxy will try reconnecting in the loop (with backoff) if TCE becomes unavailable
    // TODO: Revise this approach?
    let (tce_proxy_worker, source_head_certificate_id) = match TceProxyWorker::new(TceProxyConfig {
        subnet_id,
        base_tce_api_url: config.base_tce_api_url.clone(),
        positions: target_subnet_stream_positions,
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
            panic!("Unable to create TCE Proxy: {e}");
        }
    };

    // Launch the certification worker for certificate production
    let certification = CertificationWorker::new(
        subnet_id,
        source_head_certificate_id,
        config.verifier,
        signing_key,
    )
    .await?;

    let mut app_context = AppContext::new(
        config,
        certification,
        subnet_runtime_proxy,
        tce_proxy_worker,
    );
    app_context.run().await
}
