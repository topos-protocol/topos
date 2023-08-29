//! Protocol implementation guts.
//!
use crate::{certification::Certification, Error, SubnetRuntimeProxyConfig};
use opentelemetry::trace::FutureExt;
use opentelemetry::Context;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{self, Duration};
use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_sequencer_subnet_client::{self, SubnetClient, SubnetClientListener};
use tracing::{debug, error, field, info, info_span, instrument, warn, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Arbitrary tick duration for fetching new finalized blocks
const SUBNET_BLOCK_TIME: Duration = Duration::new(2, 0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authorities {
    // TODO: proper dependencies to block type etc
}

#[derive(Debug, Clone)]
pub enum SubnetRuntimeProxyEvent {
    /// New certificate is generated
    NewCertificate {
        cert: Box<Certificate>,
        ctx: Context,
    },
    /// New set of authorities in charge of the threshold signature
    NewEra(Vec<Authorities>),
}

#[derive(Debug)]
pub enum SubnetRuntimeProxyCommand {
    /// Upon receiving a new delivered Certificate from the TCE
    OnNewDeliveredCertificate {
        certificate: Certificate,
        position: u64,
        ctx: Context,
    },
}

pub struct SubnetRuntimeProxy {
    pub commands_channel: mpsc::Sender<SubnetRuntimeProxyCommand>,
    pub events_subscribers: Vec<mpsc::Sender<SubnetRuntimeProxyEvent>>,
    pub config: SubnetRuntimeProxyConfig,
    pub certification: Arc<Mutex<Certification>>,
    command_task_shutdown: mpsc::Sender<oneshot::Sender<()>>,
    block_task_shutdown: mpsc::Sender<oneshot::Sender<()>>,
    source_head_certificate_id_sender: Option<oneshot::Sender<Option<CertificateId>>>,
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
        let (command_sender, mut command_rcv) = mpsc::channel::<SubnetRuntimeProxyCommand>(256);
        let ws_runtime_endpoint = format!("ws://{}/ws", &config.endpoint);
        let http_runtime_endpoint = format!("http://{}", &config.endpoint);
        let subnet_contract_address = Arc::new(config.subnet_contract_address.clone());
        let (command_task_shutdown_channel, mut command_task_shutdown) =
            mpsc::channel::<oneshot::Sender<()>>(1);
        let (block_task_shutdown_channel, mut block_task_shutdown) =
            mpsc::channel::<oneshot::Sender<()>>(1);
        let (source_head_certificate_id_sender, source_head_certificate_id_received) =
            oneshot::channel();

        let certification = Certification::new(
            &config.subnet_id,
            None,
            config.verifier,
            signing_key.clone(),
        )?;

        let runtime_proxy = Arc::new(Mutex::from(Self {
            commands_channel: command_sender,
            events_subscribers: Vec::new(),
            config: config.clone(),
            command_task_shutdown: command_task_shutdown_channel,
            block_task_shutdown: block_task_shutdown_channel,
            certification: certification.clone(),
            source_head_certificate_id_sender: Some(source_head_certificate_id_sender),
        }));

        // Runtime block task
        {
            let runtime_proxy = runtime_proxy.clone();
            let subnet_contract_address = subnet_contract_address.clone();
            tokio::spawn(async move {
                {
                    // To start producing certificates, we need to know latest delivered or pending certificate id from TCE
                    // It could be also genesis certificate (also retrievable from TCE)
                    // Lock certification component and wait until we acquire first certificate id for this network
                    let mut certification = certification.lock().await;
                    if certification.last_certificate_id.is_none() {
                        info!("Waiting for the source head certificate id to continue with certificate generation");
                        // Wait for last_certificate_id retrieved on TCE component setup
                        match source_head_certificate_id_received.await {
                            Ok(last_certificate_id) => {
                                info!(
                                    "Source head certificate id received {:?}",
                                    last_certificate_id.map(|id| id.to_string())
                                );
                                // Certificate generation is now ready to run
                                certification.last_certificate_id = last_certificate_id;
                            }
                            Err(e) => {
                                panic!("Failed to get source head certificate, unable to proceed with certificate generation: {e}")
                            }
                        }
                    }
                }

                // Establish the connection with the Subnet
                let subnet_listener: Option<SubnetClientListener> = loop {
                    tokio::select! {
                        // Create subnet client
                        Ok(client) = topos_sequencer_subnet_client::connect_to_subnet_listener_with_retry(
                            ws_runtime_endpoint.as_str(),
                            subnet_contract_address.as_str(),
                        ) => {
                            break Some(client);
                        }
                        _ = block_task_shutdown.recv() => {
                            break None;
                        }
                    }
                };

                let mut interval = time::interval(SUBNET_BLOCK_TIME);

                let mut subnet_listener = subnet_listener.expect("subnet listener");

                let shutdowned: Option<oneshot::Sender<()>> = loop {
                    tokio::select! {
                        _ = interval.tick() => {

                            match subnet_listener
                                .get_next_finalized_block()
                                .await
                            {
                                Ok(block_info) => {
                                    let block_number = block_info.number;
                                    info!("Successfully fetched the finalized block {block_number} from the subnet runtime");

                                    let mut certification = certification.lock().await;

                                    // Update certificate block history
                                    certification.append_blocks(vec![block_info]);

                                    let new_certificates = match certification.generate_certificates().await {
                                        Ok(certificates) => certificates,
                                        Err(e) => {
                                            error!("Unable to generate certificates: {e}");
                                            break None;
                                        }
                                    };

                                    debug!("Generated new certificates {new_certificates:?}");

                                    for cert in new_certificates {
                                        Self::send_new_certificate(runtime_proxy.clone(), cert).await
                                    }
                                }
                                Err(topos_sequencer_subnet_client::Error::BlockNotAvailable(block_number)) => {
                                    error!("New block {block_number} not yet available, trying again soon");
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
            })
        };

        // Runtime command task
        tokio::spawn(async move {
            // Establish the connection with the Subnet
            let mut subnet_client: Option<SubnetClient> = loop {
                tokio::select! {
                    // Create subnet client
                    Ok(client) = topos_sequencer_subnet_client::connect_to_subnet_with_retry(
                        http_runtime_endpoint.as_ref(),
                        Some(signing_key.clone()),
                        subnet_contract_address.as_str(),
                    ) => {
                        info!("Connected to subnet node {}", &http_runtime_endpoint);
                        break Some(client);
                    }
                    _ = command_task_shutdown.recv() => {
                        break None;
                    }
                }
            };

            let shutdowned: Option<oneshot::Sender<()>> = loop {
                tokio::select! {
                    // Poll runtime proxy commands channel
                    cmd = command_rcv.recv() => {
                        Self::on_command(&config, subnet_client.as_mut().unwrap(), cmd).await;
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
        });

        Ok(runtime_proxy)
    }

    /// Dispatch newly generated certificate to TCE client
    #[instrument(name = "NewCertificate", fields(certification = field::Empty, source_subnet_id = field::Empty, certificate_id = field::Empty))]
    async fn send_new_certificate(
        subnet_runtime_proxy: Arc<Mutex<SubnetRuntimeProxy>>,
        cert: Certificate,
    ) {
        let mut runtime_proxy = subnet_runtime_proxy.lock().await;
        Span::current().record("certificate_id", cert.id.to_string());
        Span::current().record("source_subnet_id", cert.source_subnet_id.to_string());

        runtime_proxy
            .send_out_event(SubnetRuntimeProxyEvent::NewCertificate {
                cert: Box::new(cert),
                ctx: Span::current().context(),
            })
            .with_current_context()
            .instrument(Span::current())
            .await;
    }

    /// Send certificate to target subnet Topos Core contract for verification
    async fn push_certificate(
        runtime_proxy_config: &SubnetRuntimeProxyConfig,
        subnet_client: &mut SubnetClient,
        cert: &Certificate,
        position: u64,
    ) -> Result<Option<String>, Error> {
        debug!(
            "Pushing certificate with id {} to target subnet {}, tcc {}",
            cert.id, runtime_proxy_config.subnet_id, runtime_proxy_config.subnet_contract_address,
        );
        let receipt = subnet_client.push_certificate(cert, position).await?;
        debug!("Push certificate transaction receipt: {:?}", &receipt);
        let tx_hash =
            receipt.map(|tx_receipt| "0x".to_string() + &hex::encode(tx_receipt.transaction_hash));
        Ok(tx_hash)
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
                    let span_subnet_runtime_proxy = info_span!("Subnet Runtime Proxy");
                    span_subnet_runtime_proxy.set_parent(ctx);

                    async {
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

                        let span_push_certificate = info_span!("Subnet push certificate call");

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
                                    "Successfully pushed the Certificate {} to target subnet with tx hash {:?}",
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
                    .with_context(span_subnet_runtime_proxy.context())
                    .instrument(span_subnet_runtime_proxy)
                    .await
                }
            },
            _ => {
                warn!("Empty command was passed");
            }
        }
    }

    async fn send_out_event(&mut self, evt: SubnetRuntimeProxyEvent) {
        for tx in &self.events_subscribers {
            if let Err(e) = tx.send(evt.clone()).await {
                error!("Unable to send subnet runtime proxy event: {e}");
            }
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

    pub async fn set_source_head_certificate_id(
        &mut self,
        source_head_certificate_id: Option<CertificateId>,
    ) -> Result<(), Error> {
        self.source_head_certificate_id_sender
            .take()
            .ok_or_else(|| {
                Error::SourceHeadCertChannelError(
                    "source head certificate id was previously set".to_string(),
                )
            })?
            .send(source_head_certificate_id)
            .map_err(|_| Error::SourceHeadCertChannelError("channel error".to_string()))
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
