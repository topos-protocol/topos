pub mod subnet_contract;

use crate::subnet_contract::{
    create_topos_core_contract_from_json, parse_events_from_json, parse_events_from_log,
};
use thiserror::Error;
use topos_core::uci::CertificateId;
use topos_sequencer_types::{BlockInfo, Certificate};
use tracing::{debug, error, info};
use web3::ethabi::Token;
use web3::futures::StreamExt;
use web3::transports::{Http, WebSocket};
use web3::types::{BlockId, BlockNumber, H160, U256, U64};

#[derive(Debug, Error)]
pub enum Error {
    #[error("new finalized block not available")]
    BlockNotAvailable,
    #[error("invalid block number")]
    InvalidBlockNumber,
    #[error("data not available")]
    DataNotAvailable,
    #[error("failed mutable cast")]
    MutableCastFailed,
    #[error("json error: {source}")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
    #[error("json parse error")]
    JsonParseError,
    #[error("hex data decoding error: {source}")]
    HexDecodingError {
        #[from]
        source: hex::FromHexError,
    },
    #[error("ethereum abi error: {source}")]
    EthAbiError {
        #[from]
        source: web3::ethabi::Error,
    },
    #[error("ethereum contract error: {source}")]
    EthContractError {
        #[from]
        source: web3::contract::Error,
    },
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },
    #[error("failed to parse integer value from string: {source}")]
    IntParseError {
        #[from]
        source: std::num::ParseIntError,
    },
    #[error("failed to convert slice: {source}")]
    SliceConversionError {
        #[from]
        source: std::array::TryFromSliceError,
    },
    #[error("failed to parse int value from string: {source}")]
    FromStrRadixError {
        #[from]
        source: web3::ethabi::ethereum_types::FromStrRadixErr,
    },
    #[error("error constructing key: {source}")]
    KeyDerivationError {
        #[from]
        source: secp256k1::Error,
    },
    #[error("error with signing ethereum transaction")]
    EthereumTxSignError,
    #[error("web3 error: {source}")]
    Web3Error {
        #[from]
        source: web3::Error,
    },
    #[error("invalid web3 subscription")]
    InvalidWeb3Subscription,
    #[error("input output error: {source}")]
    InputOutputError {
        #[from]
        source: std::io::Error,
    },
    #[error("invalid certificate id")]
    InvalidCertificateId,
}

// Subnet client for listening events from subnet node
pub struct SubnetClientListener {
    web3_client: web3::Web3<web3::transports::WebSocket>,
    latest_block: Option<u64>,
    block_subscription: Option<
        web3::api::SubscriptionStream<web3::transports::WebSocket, web3::types::BlockHeader>,
    >,
    #[allow(dead_code)]
    contract: web3::contract::Contract<WebSocket>,
    events: Vec<web3::ethabi::Event>,
}

impl SubnetClientListener {
    /// Initialize a new Subnet client
    pub async fn new(ws_subnet_endpoint: &str, contract_address: &str) -> Result<Self, Error> {
        info!(
            "Connecting to subnet node at endpoint: {}",
            ws_subnet_endpoint
        );
        let transport = web3::transports::WebSocket::new(ws_subnet_endpoint).await?;
        let web3 = web3::Web3::new(transport);

        // Initialize Topos Core Contract from json abi
        let contract = create_topos_core_contract_from_json(&web3, contract_address)?;
        // List of events that this contract could create
        let events = parse_events_from_json()?;

        Ok(SubnetClientListener {
            web3_client: web3,
            latest_block: None,
            block_subscription: None,
            contract,
            events,
        })
    }

    /// Subscribe and listen to runtime finalized blocks
    pub async fn get_next_finalized_block(
        &mut self,
        _subnet_contract: &str,
    ) -> Result<BlockInfo, Error> {
        // TODO keep latest read block in storage not to lose history and sync previous blocks
        let sub = if self.block_subscription.is_none() {
            self.block_subscription = Some(
                self.web3_client
                    .eth_subscribe()
                    .subscribe_new_heads()
                    .await?,
            );
            self.block_subscription
                .as_mut()
                .ok_or(Error::InvalidWeb3Subscription)?
        } else {
            self.block_subscription
                .as_mut()
                .ok_or(Error::InvalidWeb3Subscription)?
        };

        let block_header = match sub.next().await {
            Some(value) => match value {
                Ok(block_header) => block_header,
                Err(err) => {
                    return Err(Error::Web3Error { source: err });
                }
            },
            None => return Err(Error::BlockNotAvailable),
        };
        debug!("Read block header of block {:?}", block_header.number);

        let new_block_number: u64 = match block_header.number {
            Some(number) => number.as_u64(),
            None => return Err(Error::InvalidBlockNumber),
        };
        let block_number = BlockNumber::Number(U64::from(new_block_number));

        // Get next block
        let block = match self
            .web3_client
            .eth()
            .block(BlockId::Number(block_number))
            .await?
        {
            Some(block) => {
                self.latest_block = Some(new_block_number + 1);
                block
            }
            None => return Err(Error::BlockNotAvailable),
        };

        // Take out relevant block data
        // TODO decide which data to keep
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(block.state_root.as_bytes());
        data.extend_from_slice(block.transactions_root.as_bytes());
        data.extend_from_slice(block.receipts_root.as_bytes());

        // Parse events
        let signatures = self
            .events
            .iter()
            .map(|e| e.signature())
            .collect::<Vec<web3::types::H256>>();
        let filter = web3::types::FilterBuilder::default()
            .from_block(block_number)
            .address(vec![self.contract.address()])
            .topics(Some(signatures), None, None, None)
            .build();
        let logs = self.web3_client.eth().logs(filter).await?;
        let events = match parse_events_from_log(&self.events, logs) {
            Ok(events) => events,
            Err(e) => {
                error!("Error parsing events from log: {}", e);
                return Err(e);
            }
        };

        // Make block info result from all collected info
        let block_info = BlockInfo {
            hash: block.hash.unwrap_or_default().to_string(),
            parent_hash: block.parent_hash.to_string(),
            number: new_block_number,
            data,
            events,
        };
        info!("Fetched new finalized block: {:?}", block_info.number);
        Ok(block_info)
    }
}

// Subnet client for calling target network smart contract
pub struct SubnetClient {
    pub eth_admin_address: H160,
    //TODO use SafeSecretKey for secret key https://github.com/graphprotocol/solidity-bindgen/blob/master/solidity-bindgen/src/secrets.rs
    // or https://crates.io/crates/zeroize to prevent leaking secp256k1::SecretKey struct in stack or memory
    eth_admin_key: secp256k1::SecretKey,
    contract: web3::contract::Contract<Http>,
}

impl SubnetClient {
    /// Initialize a new Subnet client
    pub async fn new(
        http_subnet_endpoint: &str,
        eth_admin_secret_key: Vec<u8>,
        contract_address: &str,
    ) -> Result<Self, Error> {
        info!(
            "Connecting to subnet node at endpoint: {}",
            http_subnet_endpoint
        );
        let transport = web3::transports::Http::new(http_subnet_endpoint)?;
        let web3 = web3::Web3::new(transport);
        let eth_admin_address = match subnet_contract::derive_eth_address(&eth_admin_secret_key) {
            Ok(address) => address,
            Err(e) => {
                error!("Unable to derive admin addres from secret key, error instantiating subnet client: {}", e);
                return Err(e);
            }
        };

        // Initialize Topos Core Contract from json abi
        let contract = create_topos_core_contract_from_json(&web3, contract_address)?;

        Ok(SubnetClient {
            eth_admin_address,
            eth_admin_key: secp256k1::SecretKey::from_slice(&eth_admin_secret_key)?,
            contract,
        })
    }

    pub async fn push_certificate(
        &self,
        cert: &Certificate,
    ) -> Result<web3::types::TransactionReceipt, Error> {
        // TODO how to get cert position (height)? It needs to be retrieved from the TCE
        // For now use block height
        let cert_position: u64 = 0;

        let cert_id_token: Token = web3::ethabi::Token::FixedBytes(cert.id.as_array().to_vec());
        let cert_position: Token = web3::ethabi::Token::Uint(U256::from(cert_position));
        let encoded_params = web3::ethabi::encode(&[cert_id_token, cert_position]);

        let wrapped_key = web3::signing::SecretKeyRef::new(&self.eth_admin_key);
        self.contract
            .signed_call_with_confirmations(
                "pushCertificate",
                // TODO ADD APPROPRIATE CERT POSITION AS ARGUMENT
                encoded_params,
                web3::contract::Options::default(),
                1_usize,
                wrapped_key,
            )
            .await
            .map_err(|e| Error::Web3Error { source: e })
    }

    /// Ask subnet for latest pushed cert
    /// Returns latest cert id and its position
    pub async fn get_latest_delivered_cert(&self) -> Result<(CertificateId, u64), Error> {
        // Get certificate count
        // Last certificate position is certificate count - 1
        let cert_count: U256 = self
            .contract
            .query(
                "getCertificateCount",
                (),
                None,
                web3::contract::Options::default(),
                None,
            )
            .await
            .map_err(|e| Error::EthContractError { source: e })?;

        if cert_count.as_u64() == 0 {
            // No previous certificates
            info!(
                "No previous certificate pushed to smart contract {}",
                self.contract.address().to_string()
            );
            return Ok((Default::default(), 0));
        }

        let latest_cert_position: U256 = cert_count - 1;
        let latest_cert_id: web3::ethabi::FixedBytes = self
            .contract
            .query(
                "getCertIdAtIndex",
                latest_cert_position,
                None,
                web3::contract::Options::default(),
                None,
            )
            .await
            .map_err(|e| Error::EthContractError { source: e })?;

        let latest_cert_id: CertificateId = latest_cert_id
            .try_into()
            .map_err(|_| Error::InvalidCertificateId)?;
        Ok((latest_cert_id, latest_cert_position.as_u64()))
    }
}

/// Create subnet client and open connection to the subnet
/// Retry until connection is valid
// TODO implement backoff mechanism
pub async fn connect_to_subnet_with_retry(
    http_subnet_endpoint: &str,
    eth_admin_private_key: Vec<u8>,
    contract_address: &str,
) -> SubnetClient {
    let subnet_client = loop {
        match SubnetClient::new(
            http_subnet_endpoint,
            eth_admin_private_key.clone(),
            contract_address,
        )
        .await
        {
            Ok(subnet_client) => {
                break subnet_client;
            }
            Err(err) => {
                error!(
                    "Unable to instantiate http subnet client, error: {}",
                    err.to_string()
                );
                //TODO use backoff mechanism instead of fixed delay
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
        };
    };

    subnet_client
}
