#![allow(clippy::too_many_arguments)]

pub mod subnet_contract;

use codec::Encode;
use ethereum_tx_sign::{LegacyTransaction, Transaction};
use jsonrpsee_types::ParamsSer;
use subxt::rpc::JsonValue;
use subxt::{
    rpc::ClientT, rpc::Subscription, ClientBuilder, DefaultConfig, PolkadotExtrinsicParams,
    RpcClient,
};
use thiserror::Error;
use topos_sequencer_types::{BlockInfo, SubnetEvent};

#[subxt::subxt(runtime_metadata_path = "./artifacts/metadata.scale")]
pub mod runtime {}

pub type Header = subxt::sp_runtime::generic::Header<u32, subxt::sp_runtime::traits::BlakeTwo256>;

/// Type for the Runtime block finalization subscription
pub type RpcBlockSubscription = Option<Subscription<Header>>;

/// Type for the Runtime Api
pub type RuntimeApi = runtime::RuntimeApi<
    subxt::DefaultConfig,
    subxt::extrinsic::BaseExtrinsicParams<subxt::DefaultConfig, subxt::extrinsic::PlainTip>,
>;

const ETHEREUM_EVENT_SENT: &str =
    "0x18735a4200b08be607f3f32b3575be6a100ef3df2a768d4e8e2e267ff34e8849"; // Keccak256 from Sent(uint64,uint256,address,address,address,uint256)

#[derive(Debug, Error)]
pub enum Error {
    #[error("new finalized block not available")]
    BlockNotAvailable,
    #[error("data not available")]
    DataNotAvailable,
    #[error("failed mutable cast")]
    MutableCastFailed,
    #[error("error in rpc communication: {source}")]
    RpcError {
        #[from]
        source: subxt::rpc::RpcError,
    },
    #[error("json error: {source}")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
    #[error("json parse error")]
    JsonParseError,
    #[error("subxt error: {source}")]
    SubxtError {
        #[from]
        source: subxt::GenericError<std::convert::Infallible>,
    },
    #[error("hex data decoding error: {source}")]
    HexDecodingError {
        #[from]
        source: hex::FromHexError,
    },
    #[error("etereum abi error: {source}")]
    EthError {
        #[from]
        source: ethabi::Error,
    },
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },
    #[error("failed to parse integer value from string: {source}")]
    IntParseError {
        #[from]
        source: std::num::ParseIntError,
    },
    #[error("failed to parse int value from string: {source}")]
    FromStrRadixError {
        #[from]
        source: ethabi::ethereum_types::FromStrRadixErr,
    },
    #[error("error constructing key: {source}")]
    KeyDerivationError {
        #[from]
        source: secp256k1::Error,
    },
    #[error("error with signing ethereum transaction")]
    EthereumTxSignError,
}

fn parse_subxt_subnet_event(event_data: JsonValue) -> Result<Vec<SubnetEvent>, Error> {
    let mut events = Vec::new();
    for json_event in event_data.as_array().ok_or(Error::JsonParseError)? {
        let json_event = json_event.as_object().ok_or(Error::JsonParseError)?;
        //Parse json topics, first topic is encoded name of event
        let topics = json_event
            .get("topics")
            .ok_or(Error::JsonParseError)?
            .as_array()
            .ok_or(Error::JsonParseError)?;

        // If topics are non existing, this is anonymous event so return error
        if topics.is_empty() {
            log::error!("Subnet event topic parsing failed, event type not available");
            return Err(Error::JsonParseError);
        }

        // Find event type and parse event data fields
        if topics[0].to_string().replace('\"', "") == ETHEREUM_EVENT_SENT {
            //Parse event data from json
            let event_data = json_event
                .get("data")
                .ok_or(Error::JsonParseError)?
                .to_string()
                .replace('\"', "");
            // Decode hex encoded string data to binary array form
            let event_data = hex::decode(&event_data[2..])?;
            // Decode event data fields
            let event_arguments = ethabi::decode(
                &[
                    ethabi::ParamType::Uint(64),  // target subnet id
                    ethabi::ParamType::Uint(256), // target token id
                    ethabi::ParamType::Address,   // target contract address (not used)
                    ethabi::ParamType::Address,   // sender address
                    ethabi::ParamType::Address,   // recepient address
                    ethabi::ParamType::Uint(256), // amount
                ],
                &event_data,
            )?;

            // Create subnet event from parsed data
            events.push(SubnetEvent::SendToken {
                target_subnet_id: if let ethabi::Token::Uint(value) = event_arguments[0] {
                    value.to_string()
                } else {
                    return Err(Error::InvalidArgument {
                        message: "invalid subnet id event argument".to_string(),
                    });
                },
                asset_id: if let ethabi::Token::Uint(value) = event_arguments[1] {
                    value
                } else {
                    return Err(Error::InvalidArgument {
                        message: "invalid target token id event argument".to_string(),
                    });
                },
                sender_addr: if let ethabi::Token::Address(address) = event_arguments[3] {
                    address.to_string()
                } else {
                    return Err(Error::InvalidArgument {
                        message: "invalid sender address".to_string(),
                    });
                },
                recipient_addr: if let ethabi::Token::Address(address) = event_arguments[4] {
                    address.to_string()
                } else {
                    return Err(Error::InvalidArgument {
                        message: "invalid destination subnet recipient".to_string(),
                    });
                },
                amount: if let ethabi::Token::Uint(value) = event_arguments[5] {
                    value
                } else {
                    return Err(Error::InvalidArgument {
                        message: "invalid amount event argument".to_string(),
                    });
                },
            })
        } else {
            log::warn!(
                "Unknown subnet event type {}, ignored",
                topics[0].to_string().as_str()
            );
        }
    }
    Ok(events)
}

// A struct to hold the Subxt api
pub struct Subxt {
    pub api: RuntimeApi,
    pub sub: RpcBlockSubscription,
    pub rpc_client: std::sync::Arc<RpcClient>,
    eth_admin_key: Vec<u8>,
    pub eth_admin_address: String,
}

impl Subxt {
    /// Initialize a new Subxt Api client
    pub async fn new(substrate_endpoint: &str, eth_admin_key: Vec<u8>) -> Result<Self, Error> {
        let client = ClientBuilder::new()
            .set_url(substrate_endpoint)
            .build()
            .await;

        let client = match client {
            Ok(c) => c,
            Err(e) => {
                log::error!(
                    "Error connecting to substrate subnet endpoint {:?}, error: {e:?}",
                    substrate_endpoint
                );
                return Err(e.into());
            }
        };

        let rpc_client: std::sync::Arc<RpcClient> = client.rpc().client.clone();

        let api = client.to_runtime_api::<runtime::RuntimeApi<DefaultConfig, PolkadotExtrinsicParams<DefaultConfig>>>();

        let sub = match api.client.rpc().subscribe_finalized_blocks().await {
            Ok(sub) => Some(sub),
            Err(e) => {
                log::error!("Failed to subscribe to finalized blocks {e:?}");
                return Err(e.into());
            }
        };

        let eth_admin_address = match subnet_contract::derive_eth_address(&eth_admin_key) {
            Ok(address) => address,
            Err(e) => {
                log::error!("Unable to derive admin addres from secret key, error instantiating subxt client: {}", e);
                return Err(e);
            }
        };

        Ok(Subxt {
            api,
            sub,
            rpc_client,
            eth_admin_key,
            eth_admin_address,
        })
    }

    /// Subscribe and listen to runtime finalized blocks
    pub async fn get_next_finalized_block(
        &mut self,
        subnet_contract: &str,
    ) -> Result<BlockInfo, Error> {
        let sub = self.sub.as_mut().ok_or(Error::MutableCastFailed)?;

        if let Some(header) = sub.next().await {
            let header: Header = header?;
            let block_data = self.get_block_data(&header).await?;

            // Get events data
            let events: Vec<SubnetEvent> = {
                let param_json_str = format!(
                    "{{\"fromBlock\":\"{}\",\"toBlock\":\"{}\",\"address\":\"{}\"}}",
                    header.number, header.number, subnet_contract
                );
                let log_params = ParamsSer::Array(vec![serde_json::from_str(&param_json_str)?]);
                let event_data: serde_json::Value = self
                    .rpc_client
                    .request("eth_getLogs", Some(log_params))
                    .await?;
                log::trace!(
                    "Fetched logs for block: {}:\n {:#?}",
                    header.number,
                    event_data
                );
                parse_subxt_subnet_event(event_data)?
            };
            log::debug!("Events received in block {}: {:?}", header.number, events);

            let block_info = BlockInfo {
                hash: header.hash().to_string(),
                parent_hash: header.parent_hash.to_string(),
                number: header.number,
                data: block_data,
                events,
            };

            log::debug!("Fetched new finalized block: {:?}", block_info.number);

            Ok(block_info)
        } else {
            Err(Error::BlockNotAvailable)
        }
    }

    /// Gets a block raw data from the runtime client
    pub async fn get_block_data(&mut self, header: &Header) -> Result<Vec<u8>, Error> {
        let data = self
            .api
            .client
            .rpc()
            .block(Some(header.hash()))
            .await?
            .ok_or(Error::BlockNotAvailable)?;
        Ok(data.block.extrinsics.encode())
    }

    pub async fn get_eth_nonce(&self, address: &str) -> Result<u128, Error> {
        // Get transaction count from native blockchain for particular account
        let nonce_address = format!("\"{}\"", &address[2..]);
        let log_params = ParamsSer::Array(vec![
            serde_json::from_str(&nonce_address)?,
            serde_json::from_str("\"pending\"")?,
        ]);
        let nonce: serde_json::Value = self
            .rpc_client
            .request("eth_getTransactionCount", Some(log_params))
            .await?;
        let nonce = u128::from_str_radix(&nonce.as_str().unwrap()[2..], 16)?;
        Ok(nonce)
    }

    pub async fn call_subnet_contract(&self, tx: LegacyTransaction) -> Result<String, Error> {
        // Send transaction using eth_rawTransaction
        let ecdsa = tx
            .ecdsa(&self.eth_admin_key)
            .map_err(|_e| Error::EthereumTxSignError)?;
        let tx_bytes = tx.sign(&ecdsa);
        let param_json = serde_json::json!("0x".to_string() + &hex::encode(tx_bytes));
        let transaction_params = ParamsSer::Array(vec![param_json]);

        let tx_hash = self
            .rpc_client
            .request::<serde_json::Value>("eth_sendRawTransaction", Some(transaction_params))
            .await?
            .to_string();
        Ok(tx_hash)
    }
}
