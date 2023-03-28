//! Protocol implementation guts.
//!

use crate::{Error, SubnetRuntimeProxyConfig};
use opentelemetry::trace::FutureExt;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{self, Duration};
use topos_core::api::checkpoints::TargetStreamPosition;
use topos_core::uci::{Certificate, SubnetId};
use topos_sequencer_subnet_client::{self, SubnetClient};
use topos_sequencer_types::{SubnetRuntimeProxyCommand, SubnetRuntimeProxyEvent};
use tracing::{debug, error, info, info_span, warn, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Arbitrary tick duration for fetching new finalized blocks
const SUBNET_BLOCK_TIME: Duration = Duration::new(2, 0);

pub struct SubnetRuntimeProxy {
    pub commands_channel: mpsc::UnboundedSender<SubnetRuntimeProxyCommand>,
    pub events_subscribers: Vec<mpsc::UnboundedSender<SubnetRuntimeProxyEvent>>,
    pub config: SubnetRuntimeProxyConfig,
    command_task_shutdown: mpsc::Sender<oneshot::Sender<()>>,
    block_task_shutdown: mpsc::Sender<oneshot::Sender<()>>,
}

impl Debug for SubnetRuntimeProxy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeProxy instance").finish()
    }
}

impl SubnetRuntimeProxy {
    pub fn spawn_new(
        config: SubnetRuntimeProxyConfig,
        signing_key: Vec<u8>,
    ) -> Result<Arc<Mutex<SubnetRuntimeProxy>>, crate::Error> {
        info!(
            "Spawning new runtime proxy, endpoint: {} ethereum contract address: {}, ",
            &config.endpoint, &config.subnet_contract_address
        );
        let (command_sender, mut command_rcv) =
            mpsc::unbounded_channel::<SubnetRuntimeProxyCommand>();
        let ws_runtime_endpoint = format!("ws://{}/ws", &config.endpoint);
        let http_runtime_endpoint = format!("http://{}", &config.endpoint);
        let subnet_contract_address = Arc::new(config.subnet_contract_address.clone());
        let (command_task_shutdown_channel, mut command_task_shutdown) =
            mpsc::channel::<oneshot::Sender<()>>(1);
        let (block_task_shutdown_channel, mut block_task_shutdown) =
            mpsc::channel::<oneshot::Sender<()>>(1);

        let runtime_proxy = Arc::new(Mutex::from(Self {
            commands_channel: command_sender,
            events_subscribers: Vec::new(),
            config: config.clone(),
            command_task_shutdown: command_task_shutdown_channel,
            block_task_shutdown: block_task_shutdown_channel,
        }));

        // Runtime block task
        {
            let runtime_proxy = runtime_proxy.clone();
            let subnet_contract_address = subnet_contract_address.clone();
            tokio::spawn(async move {
                loop {
                    // Create subnet listener
                    let mut subnet_listener =
                        match topos_sequencer_subnet_client::connect_to_subnet_listener_with_retry(
                            ws_runtime_endpoint.as_str(),
                            subnet_contract_address.as_str(),
                        )
                        .await
                        {
                            Ok(subnet_listener) => subnet_listener,
                            Err(e) => {
                                error!("Unable create subnet listener: {e}");
                                continue;
                            }
                        };

                    let mut interval = time::interval(SUBNET_BLOCK_TIME);

                    let shutdowned: Option<oneshot::Sender<()>> = loop {
                        tokio::select! {
                            _ = interval.tick() => {
                                match subnet_listener
                                    .get_next_finalized_block(&subnet_contract_address)
                                    .await
                                {
                                    Ok(block_info) => {
                                        let block_number = block_info.number;
                                        match Self::send_new_block(
                                            runtime_proxy.clone(),
                                            SubnetRuntimeProxyEvent::BlockFinalized(block_info),
                                        ).await {
                                            Ok(()) => {
                                                debug!(
                                                    "Successfully fetched the finalized block {:?} from the subnet runtime",
                                                    block_number
                                                )
                                            }
                                            Err(e) => {
                                                // TODO: Determine if task should end on some type of error
                                                error!(
                                                    "Failed to send new finalize block: {}",
                                                    e
                                                );
                                                break None;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // TODO: Determine if task should end on some type of error
                                        error!("Failed to fetch the new finalized block: {e}");
                                        break None;
                                    }
                                }
                            },
                            shutdown = block_task_shutdown.recv() => {
                                break shutdown;
                            }
                        }
                    };

                    if let Some(sender) = shutdowned {
                        info!("Shutting down subnet runtime block processing task");
                        _ = sender.send(());
                    } else {
                        warn!("Shutting down subnet runtime block processing task due to error");
                    }
                }
            })
        };

        // Runtime command task
        tokio::spawn(async move {
            loop {
                // Create subnet client
                let mut subnet_client =
                    match topos_sequencer_subnet_client::connect_to_subnet_with_retry(
                        http_runtime_endpoint.as_ref(),
                        Some(signing_key.clone()),
                        subnet_contract_address.as_str(),
                    )
                    .await
                    {
                        Ok(subnet_client) => {
                            info!("Connected to subnet node {}", &http_runtime_endpoint);
                            subnet_client
                        }
                        Err(e) => {
                            error!("Unable to connect to the subnet node: {e}");
                            continue;
                        }
                    };

                let shutdowned: Option<oneshot::Sender<()>> = loop {
                    tokio::select! {
                        // Poll runtime proxy commands channel
                        cmd = command_rcv.recv() => {
                            Self::on_command(&config, &mut subnet_client, cmd).await;
                        },
                        shutdown = command_task_shutdown.recv() => {
                            break shutdown;
                        }
                    }
                };

                if let Some(sender) = shutdowned {
                    info!("Shutting down subnet runtime command processing task");
                    _ = sender.send(());
                } else {
                    warn!("Shutting down subnet runtime command processing task due to error");
                }
            }
        });

        Ok(runtime_proxy)
    }

    async fn send_new_block(
        runtime_proxy: Arc<Mutex<SubnetRuntimeProxy>>,
        evt: SubnetRuntimeProxyEvent,
    ) -> Result<(), Error> {
        let mut runtime_proxy = runtime_proxy.lock().await;
        runtime_proxy.send_out_events(evt);
        Ok(())
    }

    /// Send certificate to target subnet Topos Core contract for verification
    async fn push_certificate(
        runtime_proxy_config: &SubnetRuntimeProxyConfig,
        subnet_client: &mut SubnetClient,
        cert: &Certificate,
        position: u64,
    ) -> Result<String, Error> {
        debug!(
            "Pushing certificate with id {} to target subnet {}, tcc {}",
            cert.id, runtime_proxy_config.subnet_id, runtime_proxy_config.subnet_contract_address,
        );
        let receipt = subnet_client.push_certificate(cert, position).await?;
        debug!("Push certificate transaction receipt: {:?}", &receipt);
        Ok("0x".to_string() + &hex::encode(receipt.transaction_hash))
    }

    async fn on_command(
        runtime_proxy_config: &SubnetRuntimeProxyConfig,
        subnet_client: &mut SubnetClient,
        mb_cmd: Option<SubnetRuntimeProxyCommand>,
    ) {
        match mb_cmd {
            Some(cmd) => match cmd {
                // Process certificate retrieved from TCE node
                SubnetRuntimeProxyCommand::OnNewDeliveredCertificate {
                    certificate,
                    position,
                    ctx,
                } => {
                    let subnet_runtime_proxy_span = info_span!("Subnet Runtime Proxy");
                    subnet_runtime_proxy_span.set_parent(ctx);
                    let span_push_certificate = info_span!(
                        parent: &subnet_runtime_proxy_span,
                        "Subnet push certificate call"
                    );
                    _ = subnet_runtime_proxy_span.entered();
                    info!(
                        "Processing certificate received from TCE, cert_id={}",
                        &certificate.id
                    );

                    // Verify certificate signature
                    // Well known subnet id is public key for certificate verification
                    // Public key of secp256k1 is 33 bytes, we are keeping last 32 bytes as subnet id
                    // Add manually first byte 0x02
                    let public_key = certificate.source_subnet_id.to_secp256k1_public_key();

                    // Verify signature of the certificate
                    match topos_crypto::signatures::verify(
                        &public_key,
                        certificate.get_payload().as_slice(),
                        certificate.signature.as_slice(),
                    ) {
                        Ok(()) => {
                            info!("Certificate {} passed verification", certificate.id)
                        }
                        Err(e) => {
                            error!("Failed to verify certificate id {}: {e}", certificate.id);
                            return;
                        }
                    }

                    // Push the Certificate to the ToposCore contract on the target subnet
                    match SubnetRuntimeProxy::push_certificate(
                        runtime_proxy_config,
                        subnet_client,
                        &certificate,
                        position,
                    )
                    .with_context(span_push_certificate.context())
                    .instrument(span_push_certificate)
                    .await
                    {
                        Ok(tx_hash) => {
                            debug!(
                                "Successfully pushed the Certificate {} to target subnet with tx hash {}",
                                &certificate.id, &tx_hash
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to push the Certificate {} to target subnet: {e}",
                                &certificate.id
                            );
                        }
                    }
                }
            },
            _ => {
                warn!("Empty command was passed");
            }
        }
    }

    fn send_out_events(&mut self, evt: SubnetRuntimeProxyEvent) {
        for tx in &self.events_subscribers {
            // FIXME: When error is returned it means that receiving side of the channel is closed
            // Thus we better remove the sender from our subscribers
            let _ = tx.send(evt.clone());
        }
    }

    /// Shutdown subnet runtime proxy tasks
    pub async fn shutdown(&self) -> Result<(), Error> {
        let (command_task_sender, command_task_receiver) = oneshot::channel();
        self.command_task_shutdown
            .send(command_task_sender)
            .await
            .map_err(Error::ShutdownCommunication)?;
        command_task_receiver
            .await
            .map_err(Error::ShutdownSignalReceiveError)?;

        let (block_task_sender, block_task_receiver) = oneshot::channel();
        self.block_task_shutdown
            .send(block_task_sender)
            .await
            .map_err(Error::ShutdownCommunication)?;
        block_task_receiver
            .await
            .map_err(Error::ShutdownSignalReceiveError)?;
        Ok(())
    }

    pub async fn get_checkpoints(&self) -> Result<Vec<TargetStreamPosition>, Error> {
        info!("Connecting to subnet to query for checkpoints...");
        let http_runtime_endpoint = format!("http://{}", &self.config.endpoint);
        // Create subnet client
        let subnet_client = match topos_sequencer_subnet_client::connect_to_subnet_with_retry(
            http_runtime_endpoint.as_ref(),
            None, // We do not need actual key here as we are just reading state
            self.config.subnet_contract_address.as_str(),
        )
        .await
        {
            Ok(subnet_client) => {
                info!(
                    "Connected to subnet node to acquire checkpoints {}",
                    &http_runtime_endpoint
                );
                subnet_client
            }
            Err(e) => {
                error!("Unable to connect to the subnet node to get checkpoints: {e}");
                return Err(Error::SubnetError { source: e });
            }
        };

        match subnet_client.get_checkpoints(&self.config.subnet_id).await {
            Ok(checkpoints) => {
                info!("Successfully retrieved the Checkpoints");
                Ok(checkpoints)
            }
            Err(e) => {
                error!(
                    "Unable to get the checkpoints for subnet {}",
                    self.config.subnet_id
                );
                Err(Error::SubnetError { source: e })
            }
        }
    }

    pub async fn get_subnet_id(endpoint: &str, contract_address: &str) -> Result<SubnetId, Error> {
        info!("Connecting to subnet to query for subnet id...");
        let http_runtime_endpoint = format!("http://{endpoint}");
        // Create subnet client
        let subnet_client = match topos_sequencer_subnet_client::connect_to_subnet_with_retry(
            http_runtime_endpoint.as_ref(),
            None, // We do not need actual key here as we are just reading state
            contract_address,
        )
        .await
        {
            Ok(subnet_client) => {
                info!(
                    "Connected to subnet node to acquire subnet id {}",
                    &http_runtime_endpoint
                );
                subnet_client
            }
            Err(e) => {
                error!("Unable to connect to the subnet node to get subnet id: {e}");
                return Err(Error::SubnetError { source: e });
            }
        };

        match subnet_client.get_subnet_id().await {
            Ok(subnet_id) => {
                info!("Successfully retrieved the subnet id for subnet: {subnet_id}");
                Ok(subnet_id)
            }
            Err(e) => {
                error!("Unable to get the subnet id {e}",);
                Err(Error::SubnetError { source: e })
            }
        }
    }
}
