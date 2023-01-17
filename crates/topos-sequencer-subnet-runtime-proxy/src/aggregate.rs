//! Protocol implementation guts.
//!

use crate::{Error, RuntimeProxyConfig};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use topos_core::uci::Certificate;
use topos_sequencer_subnet_client::{self, SubnetClient};
use topos_sequencer_types::{RuntimeProxyCommand, RuntimeProxyEvent};
use tracing::{debug, error, info, trace, warn};

pub struct RuntimeProxy {
    pub commands_channel: mpsc::UnboundedSender<RuntimeProxyCommand>,
    pub events_subscribers: Vec<mpsc::UnboundedSender<RuntimeProxyEvent>>,
    pub config: RuntimeProxyConfig,
    _tx_exit: mpsc::UnboundedSender<()>,
}

impl Debug for RuntimeProxy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeProxy instance").finish()
    }
}

impl RuntimeProxy {
    pub fn spawn_new(config: RuntimeProxyConfig) -> Result<Arc<Mutex<RuntimeProxy>>, crate::Error> {
        info!(
            "Spawning new runtime proxy, endpoint: {} ethereum contract address: {}, ",
            &config.endpoint, &config.subnet_contract_address
        );
        let (command_sender, mut command_rcv) = mpsc::unbounded_channel::<RuntimeProxyCommand>();
        let ws_runtime_endpoint = format!("ws://{}/ws", &config.endpoint);
        let http_runtime_endpoint = format!("http://{}", &config.endpoint);
        let subnet_contract_address = Arc::new(config.subnet_contract_address.clone());
        let (_tx_exit, mut rx_exit) = mpsc::unbounded_channel::<()>();

        // Get ethereum private key from keystore
        debug!(
            "Retrieving ethereum private key from keystore {:?}",
            config.keystore_file.to_str()
        );
        // To sign transactions sent to Topos core contract, use admin private key from keystore
        let eth_admin_private_key: Vec<u8> = match crate::keystore::get_private_key(
            &config.keystore_file,
            &config.keystore_password,
        ) {
            Ok(key) => key,
            Err(e) => {
                error!(
                    "unable to get ethereum private key from keystore, details: {}",
                    e
                );
                return Err(Error::from(e));
            }
        };

        let runtime_proxy = Arc::new(Mutex::from(Self {
            commands_channel: command_sender,
            events_subscribers: Vec::new(),
            _tx_exit,
            config: config.clone(),
        }));

        let _runtime_block_task = {
            let runtime_proxy = runtime_proxy.clone();
            let subnet_contract_address = subnet_contract_address.clone();
            tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_secs(6)); // arbitrary time for 1 block
                loop {
                    let mut subnet = match topos_sequencer_subnet_client::SubnetClientListener::new(
                        ws_runtime_endpoint.as_ref(),
                        subnet_contract_address.as_str(),
                    )
                    .await
                    {
                        Ok(subnet) => subnet,
                        Err(err) => {
                            error!(
                                "Unable to instantiate subnet client listener, error: {}",
                                err.to_string()
                            );
                            //TODO use backoff mechanism here
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            continue;
                        }
                    };

                    loop {
                        interval.tick().await;

                        loop {
                            match subnet
                                .get_next_finalized_block(&subnet_contract_address)
                                .await
                            {
                                Ok(block_info) => {
                                    let block_number = block_info.number;
                                    match Self::send_new_block(
                                        runtime_proxy.clone(),
                                        RuntimeProxyEvent::BlockFinalized(block_info),
                                    ) {
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
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    // todo determine if task should end on some type of error
                                    error!("failed to get new finalized block, details: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            })
        };

        let _runtime_command_task = {
            tokio::spawn(async move {
                loop {
                    let mut subnet_client = match topos_sequencer_subnet_client::SubnetClient::new(
                        http_runtime_endpoint.as_ref(),
                        eth_admin_private_key.clone(),
                        subnet_contract_address.as_str(),
                    )
                    .await
                    {
                        Ok(subnet) => subnet,
                        Err(err) => {
                            error!(
                                "Unable to instantiate http subnet client, error: {}",
                                err.to_string()
                            );
                            //TODO use backoff mechanism here
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            continue;
                        }
                    };

                    loop {
                        tokio::select! {
                            // Poll runtime proxy commands channel
                            cmd = command_rcv.recv() => {
                                Self::on_command(&config, &mut subnet_client, cmd).await;
                            },
                            Some(_) = rx_exit.recv() => {
                                break;
                            }
                        }
                    }
                }
            })
        };

        Ok(runtime_proxy)
    }

    fn send_new_block(
        runtime_proxy: Arc<Mutex<RuntimeProxy>>,
        evt: RuntimeProxyEvent,
    ) -> Result<(), Error> {
        let mut runtime_proxy = runtime_proxy.lock().map_err(|_| Error::UnlockError)?;
        runtime_proxy.send_out_events(evt);
        Ok(())
    }

    /// Send certificate to target subnet Topos Core contract for verification
    async fn push_certificate(
        runtime_proxy_config: &RuntimeProxyConfig,
        subnet_client: &mut SubnetClient,
        cert: &Certificate,
    ) -> Result<String, Error> {
        debug!(
            "Pushing certificate with id {:?} to target subnet {:?}, tcc {}",
            cert.id, runtime_proxy_config.subnet_id, runtime_proxy_config.subnet_contract_address,
        );
        let receipt = subnet_client.push_certificate(cert).await?;
        debug!("Push certificate transaction receipt: {:?}", &receipt);
        Ok("0x".to_string() + &hex::encode(receipt.transaction_hash))
    }

    async fn on_command(
        runtime_proxy_config: &RuntimeProxyConfig,
        subnet_client: &mut SubnetClient,
        mb_cmd: Option<RuntimeProxyCommand>,
    ) {
        match mb_cmd {
            Some(cmd) => match cmd {
                RuntimeProxyCommand::PushCertificate(c) => {
                    info!("New received Certificate {:?}", c);
                }
                // Process certificate retrieved from TCE node
                RuntimeProxyCommand::OnNewDeliveredTxns(cert) => {
                    info!("on_command - OnNewDeliveredTxns cert_id={:?}", &cert.id);

                    // Pass certificate to target subnet Topos core contract
                    match RuntimeProxy::push_certificate(runtime_proxy_config, subnet_client, &cert)
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
                                "Failed to push certificate id={:?} to target subnet, error details: {}",
                                &cert.id, e
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

    fn send_out_events(&mut self, evt: RuntimeProxyEvent) {
        for tx in &self.events_subscribers {
            // FIXME: When error is returned it means that receiving side of the channel is closed
            // Thus we better remove the sender from our subscribers
            let _ = tx.send(evt.clone());
        }
    }
}
