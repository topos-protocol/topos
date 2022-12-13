//! Protocol implementation guts.
//!

use crate::{Error, RuntimeProxyConfig};
use log::{debug, error, info, trace, warn};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use topos_core::uci::{Certificate, CrossChainTransaction};
use topos_sequencer_subxt_client;
use topos_sequencer_subxt_client::subnet_contract;
use topos_sequencer_subxt_client::Subxt;
use topos_sequencer_types::{RuntimeProxyCommand, RuntimeProxyEvent};

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
            &config.endpoint, &config.subnet_contract
        );
        let (command_sender, mut command_rcv) = mpsc::unbounded_channel::<RuntimeProxyCommand>();
        let runtime_endpoint = Arc::new(config.endpoint.clone());
        let subnet_contract = Arc::new(config.subnet_contract.clone());
        let subnet_id: Arc<String> = Arc::new(config.subnet_id.clone());
        let (_tx_exit, mut rx_exit) = mpsc::unbounded_channel::<()>();

        // Get ethereum private key from keystore
        debug!(
            "Retrieving ethereum private key from keystore {}",
            config.keystore_file
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
            let runtime_endpoint = runtime_endpoint.clone();
            let eth_admin_private_key = eth_admin_private_key.clone();
            tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_secs(6)); // arbitrary time for 1 block
                if let Ok(mut subxt) = topos_sequencer_subxt_client::Subxt::new(
                    runtime_endpoint.as_ref(),
                    eth_admin_private_key,
                )
                .await
                {
                    loop {
                        interval.tick().await;

                        loop {
                            match subxt.get_next_finalized_block(&subnet_contract).await {
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
                };
            })
        };

        let _runtime_command_task = {
            tokio::spawn(async move {
                if let Ok(mut subxt) = topos_sequencer_subxt_client::Subxt::new(
                    runtime_endpoint.as_ref(),
                    eth_admin_private_key,
                )
                .await
                {
                    loop {
                        tokio::select! {
                            // Poll runtime proxy commands channel
                            cmd = command_rcv.recv() => {
                                Self::on_command(&config, &mut subxt, subnet_id.as_str(), cmd).await;
                            },
                            Some(_) = rx_exit.recv() => {
                                break;
                            }
                        }
                    }
                }
                Result::<(), Error>::Ok(())
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

    /// Process asset transfer to target subnet
    /// As a result return terminal subnet topos core contract call tx hash
    /// where these cross chain transactions are processed
    async fn process_asset_transfers(
        runtime_proxy_config: &RuntimeProxyConfig,
        subxt: &mut Subxt,
        cert: &Certificate,
        txs: &Vec<&CrossChainTransaction>,
    ) -> Result<String, Error> {
        debug!(
            "Processing asset transfers for topos core contract {}",
            runtime_proxy_config.subnet_contract
        );
        // Pack transaction data (certificate and asset transfers) to Eth encoded abi call
        let transaction_data =
            topos_sequencer_subxt_client::subnet_contract::subnet_encode_mint_call(cert, txs)?;
        // Get nonce so that we can prepare eth transaction
        let nonce = subxt.get_eth_nonce(&subxt.eth_admin_address).await?;
        debug!(
            "Nonce for admin account {} is {}, preparing transaction",
            subxt.eth_admin_address, nonce
        );
        let transaction = subnet_contract::subnet_prepare_transation(
            &runtime_proxy_config.subnet_contract,
            nonce,
            &transaction_data,
        )?;
        // Perform Topos core contract mint call
        Ok(subxt.call_subnet_contract(transaction).await?)
    }

    async fn on_command(
        runtime_proxy_config: &RuntimeProxyConfig,
        subxt: &mut Subxt,
        subnet_id: &str,
        mb_cmd: Option<RuntimeProxyCommand>,
    ) {
        match mb_cmd {
            Some(cmd) => match cmd {
                RuntimeProxyCommand::PushCertificate(c) => {
                    info!("New received Certificate {:?}", c);
                }
                // Process certificate retrieved from TCE node
                RuntimeProxyCommand::OnNewDeliveredTxns(cert) => {
                    info!("on_command - OnNewDeliveredTxns cert_id={}", &cert.cert_id);
                    // Make list (by reference) of asset transfer transactions
                    let mut asset_transfer_txs: Vec<&CrossChainTransaction> = Vec::new();
                    for tx in &cert.calls {
                        if tx.terminal_subnet_id == subnet_id {
                            asset_transfer_txs.push(tx);
                        }
                    }
                    // Process all asset transfer transactions and call
                    // Topos core contract
                    match RuntimeProxy::process_asset_transfers(
                        runtime_proxy_config,
                        subxt,
                        &cert,
                        &asset_transfer_txs,
                    )
                    .await
                    {
                        Ok(tx_hash) => {
                            debug!(
                                "Successfully processed transactions {:?} with the terminal subnet transaction {} ",
                                &asset_transfer_txs, &tx_hash
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to process transactions {:?} error details: {}",
                                asset_transfer_txs, e
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
