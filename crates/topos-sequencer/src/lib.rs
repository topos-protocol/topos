use crate::app_context::AppContext;
use std::io::ErrorKind::InvalidInput;
use tokio::{
    spawn,
    sync::{
        mpsc,
        oneshot::{self, Sender},
    },
};
use tokio_util::sync::CancellationToken;
use topos_core::uci::SubnetId;
use topos_sequencer_subnet_runtime::{SubnetRuntimeProxyConfig, SubnetRuntimeProxyWorker};
use topos_tce_proxy::{worker::TceProxyWorker, TceProxyConfig};
use topos_wallet::SecretKey;
use tracing::{debug, info};

mod app_context;

#[derive(Debug)]
pub struct SequencerConfiguration {
    pub subnet_id: Option<String>,
    pub public_key: Option<Vec<u8>>,
    pub subnet_jsonrpc_endpoint: String,
    pub subnet_contract_address: String,
    pub tce_grpc_endpoint: String,
    pub signing_key: SecretKey,
    pub verifier: u32,
}

pub async fn launch(
    config: SequencerConfiguration,
    ctx_send: Sender<AppContext>,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Starting topos-sequencer application");

    // If subnetID is specified as command line argument, use it
    let subnet_id: SubnetId = if let Some(pk) = &config.public_key {
        SubnetId::try_from(&pk[1..]).unwrap()
    } else if let Some(subnet_id) = &config.subnet_id {
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
    let subnet_runtime_proxy_worker = match SubnetRuntimeProxyWorker::new(
        SubnetRuntimeProxyConfig {
            subnet_id,
            endpoint: config.subnet_jsonrpc_endpoint.clone(),
            subnet_contract_address: config.subnet_contract_address.clone(),
            source_head_certificate_id: None, // Must be acquired later after TCE proxy is connected
            verifier: config.verifier,
        },
        config.signing_key.clone(),
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
    let target_subnet_stream_positions = match subnet_runtime_proxy_worker.get_checkpoints().await {
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
        base_tce_api_url: config.tce_grpc_endpoint.clone(),
        positions: target_subnet_stream_positions,
    })
    .await
    {
        Ok((tce_proxy_worker, source_head_certificate)) => {
            info!(
                "TCE proxy client is starting for the source subnet {:?} from the head {:?}",
                subnet_id, source_head_certificate
            );
            let source_head_certificate_id =
                source_head_certificate.map(|(cert, position)| (cert.id, position));
            (tce_proxy_worker, source_head_certificate_id)
        }
        Err(e) => {
            panic!("Unable to create TCE Proxy: {e}");
        }
    };

    // Set source head certificate to know from where to
    // start producing certificates
    if let Err(e) = subnet_runtime_proxy_worker
        .set_source_head_certificate_id(source_head_certificate_id)
        .await
    {
        panic!("Unable to set source head certificate id: {e}");
    }

    let _ = ctx_send.send(AppContext::new(
        config,
        subnet_runtime_proxy_worker,
        tce_proxy_worker,
    ));
    Ok(())
}

pub async fn run(
    config: SequencerConfiguration,
    shutdown: (CancellationToken, mpsc::Sender<()>),
) -> Result<(), Box<dyn std::error::Error>> {
    let shutdown_appcontext = shutdown.clone();

    let (ctx_send, mut ctx_recv) = oneshot::channel::<AppContext>();

    let launching = spawn(async move {
        let _ = launch(config, ctx_send).await;
    });

    let app_context: Option<AppContext> = tokio::select! {
        app = &mut ctx_recv => {
            Some(app.unwrap())
        },

        // Shutdown signal
        _ = shutdown.0.cancelled() => {
            info!("Stopping Sequencer launch...");
            drop(shutdown.1);
            launching.abort();
            None
        }
    };

    if let Some(mut app) = app_context {
        app.run(shutdown_appcontext).await;
    }

    info!("Exited sequencer");

    Ok(())
}
