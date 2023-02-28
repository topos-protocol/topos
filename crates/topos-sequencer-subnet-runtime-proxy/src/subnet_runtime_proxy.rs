//! Protocol implementation guts.
//!

use crate::{Error, SubnetRuntimeProxyConfig};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{self, Duration};
use topos_core::api::checkpoints::TargetStreamPosition;
use topos_core::uci::Certificate;
use topos_sequencer_subnet_client::{self, SubnetClient};
use topos_sequencer_types::{SubnetRuntimeProxyCommand, SubnetRuntimeProxyEvent};
use tracing::{debug, error, info, trace, warn};

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

        // Get ethereum private key from keystore
        debug!(
            "Retrieving ethereum private key from subnet node {:?}",
            config.subnet_data_dir.to_str()
        );

        let key_file_path = std::path::PathBuf::from(
            &(config
                .subnet_data_dir
                .to_str()
                .unwrap_or_default()
                .to_string()
                + crate::keystore::SUBNET_NODE_VALIDATOR_KEY_FILE_PATH),
        );

        // To sign transactions sent to Topos core contract, use admin private key from keystore
        //TODO handle this key in more secure way (e.g. use SafeSecretKey)
        let eth_admin_private_key: Vec<u8> =
            match crate::keystore::get_private_key(&key_file_path, None) {
                Ok(key) => key,
                Err(e) => {
                    error!(
                        "unable to get ethereum private key from keystore, details: {}",
                        e
                    );
                    return Err(e);
                }
            };

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
                                error!("Unable create subnet listener, details: {e}");
                                continue;
                            }
                        };

                    let mut interval = time::interval(SUBNET_BLOCK_TIME); // arbitrary time for 1 block

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
                                                trace!(
                                                    "Finalized block {:?} successfully sent",
                                                    block_number
                                                )
                                            }
                                            Err(e) => {
                                                // todo determine if task should end on some type of error
                                                error!(
                                                    "failed to send new finalize block, details: {}",
                                                    e
                                                );
                                                break None;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // todo determine if task should end on some type of error
                                        error!("failed to get new finalized block, details: {}", e);
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
                        Some(eth_admin_private_key.clone()),
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
        cert_position: u64,
    ) -> Result<String, Error> {
        debug!(
            "Pushing certificate with id {:?} to target subnet {:?}, tcc {}",
            cert.id, runtime_proxy_config.subnet_id, runtime_proxy_config.subnet_contract_address,
        );
        let receipt = subnet_client.push_certificate(cert, cert_position).await?;
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
                SubnetRuntimeProxyCommand::PushCertificate(c) => {
                    info!("New received Certificate {:?}", c);
                }
                // Process certificate retrieved from TCE node
                SubnetRuntimeProxyCommand::OnNewDeliveredTxns(cert) => {
                    info!("on_command - OnNewDeliveredTxns cert_id={:?}", &cert.id);

                    let cert_position = 0; //TODO Retrieve cert position from TCE node

                    // Pass certificate to target subnet Topos core contract
                    match SubnetRuntimeProxy::push_certificate(
                        runtime_proxy_config,
                        subnet_client,
                        &cert,
                        cert_position,
                    )
                    .await
                    {
                        Ok(tx_hash) => {
                            debug!(
                                "Successfully pushed certificate id={:?} to target subnet with tx hash {} ",
                                &cert.id, &tx_hash
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to push certificate id={:?} to target subnet: {e}",
                                &cert.id
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
                info!("Connected to subnet node {}", &http_runtime_endpoint);
                subnet_client
            }
            Err(e) => {
                error!("Unable to connect to the subnet node: {e}");
                return Err(Error::SubnetError { source: e });
            }
        };

        match subnet_client.get_checkpoints(&self.config.subnet_id).await {
            Ok(checkpoints) => {
                info!("Checkpoints successfully retrieved");
                Ok(checkpoints)
            }
            Err(e) => {
                error!(
                    "Unable to get checkpoints for subnet {:?}",
                    self.config.subnet_id
                );
                Err(Error::SubnetError { source: e })
            }
        }
    }
}
