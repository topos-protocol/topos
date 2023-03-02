pub mod subnet_contract;

use crate::subnet_contract::{
    create_topos_core_contract_from_json, parse_events_from_json, parse_events_from_log,
};
use topos_core::api::checkpoints::TargetStreamPosition;
use topos_sequencer_types::{BlockInfo, Certificate};
use tracing::{debug, error, info};
use web3::ethabi::Token;
use web3::futures::StreamExt;
use web3::transports::{Http, WebSocket};
use web3::types::{BlockId, BlockNumber, H160, U256, U64};

const PUSH_CERTIFICATE_GAS_LIMIT: u64 = 1000000;

#[derive(Debug, thiserror::Error)]
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
    #[error("invalid secret key")]
    InvalidKey,
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
    #[error("invalid checkpoints data")]
    InvalidCheckpointsData,
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
            state_root: block.state_root.0,
            tx_root_hash: block.transactions_root.0,
            events,
        };
        info!("Fetched new finalized block: {:?}", block_info.number);
        Ok(block_info)
    }
}

/// Create subnet client listener and open connection to the subnet
/// Retry until connection is valid
pub async fn connect_to_subnet_listener_with_retry(
    ws_runtime_endpoint: &str,
    subnet_contract_address: &str,
) -> Result<SubnetClientListener, crate::Error> {
    info!(
        "Connecting to subnet endpoint to listen events from {} using backoff strategy...",
        ws_runtime_endpoint
    );

    let op = || async {
        // Create subnet listener
        match SubnetClientListener::new(ws_runtime_endpoint, subnet_contract_address).await {
            Ok(subnet_listener) => Ok(subnet_listener),
            Err(e) => {
                error!("Unable to instantiate the subnet client listener: {e}");
                Err(new_subnet_client_proxy_backoff_err(e))
            }
        }
    };

    backoff::future::retry(backoff::ExponentialBackoff::default(), op).await
}

// Subnet client for calling target network smart contract
pub struct SubnetClient {
    pub eth_admin_address: H160,
    //TODO use SafeSecretKey for secret key https://github.com/graphprotocol/solidity-bindgen/blob/master/solidity-bindgen/src/secrets.rs
    // or https://crates.io/crates/zeroize to prevent leaking secp256k1::SecretKey struct in stack or memory
    eth_admin_key: Option<secp256k1::SecretKey>,
    contract: web3::contract::Contract<Http>,
}

impl SubnetClient {
    /// Initialize a new Subnet client
    pub async fn new(
        http_subnet_endpoint: &str,
        eth_admin_secret_key: Option<Vec<u8>>,
        contract_address: &str,
    ) -> Result<Self, Error> {
        info!(
            "Connecting to subnet node at endpoint: {}",
            http_subnet_endpoint
        );
        let transport = web3::transports::Http::new(http_subnet_endpoint)?;
        let web3 = web3::Web3::new(transport);

        let eth_admin_key = match eth_admin_secret_key.as_ref() {
            Some(eth_admin_secret_key) => {
                Some(secp256k1::SecretKey::from_slice(eth_admin_secret_key)?)
            }
            None => None,
        };

        let eth_admin_address = if let Some(eth_admin_secret_key) = eth_admin_secret_key {
            match subnet_contract::derive_eth_address(&eth_admin_secret_key) {
                Ok(address) => address,
                Err(e) => {
                    error!("Unable to derive admin address from secret key, error instantiating subnet client: {}", e);
                    return Err(e);
                }
            }
        } else {
            Default::default()
        };

        // Initialize Topos Core Contract from json abi
        let contract = create_topos_core_contract_from_json(&web3, contract_address)?;

        Ok(SubnetClient {
            eth_admin_address,
            eth_admin_key,
            contract,
        })
    }

    pub async fn push_certificate(
        &self,
        cert: &Certificate,
        cert_position: u64,
    ) -> Result<web3::types::TransactionReceipt, Error> {
        let prev_cert_id: Token = Token::FixedBytes(cert.prev_id.as_array().to_vec());
        let source_subnet_id: Token = Token::FixedBytes(cert.source_subnet_id.into());
        let state_root: Token = Token::FixedBytes(cert.state_root.to_vec());
        let tx_root: Token = Token::FixedBytes(cert.tx_root_hash.to_vec());
        let target_subnets: Token = Token::Array(
            cert.target_subnets
                .iter()
                .map(|target_subnet| Token::FixedBytes((*target_subnet).into()))
                .collect::<Vec<Token>>(),
        );
        let verifier = Token::Uint(U256::from(cert.verifier));
        let cert_id: Token = Token::FixedBytes(cert.id.as_array().to_vec());
        let stark_proof: Token = Token::Bytes(cert.proof.clone());
        let signature: Token = Token::Bytes(cert.signature.clone());
        let cert_position: Token = Token::Uint(U256::from(cert_position));
        let encoded_params = web3::ethabi::encode(&[
            prev_cert_id,
            source_subnet_id,
            state_root,
            tx_root,
            target_subnets,
            verifier,
            cert_id,
            stark_proof,
            signature,
        ]);

        let eth_admin_key = &self.eth_admin_key.ok_or(Error::InvalidKey)?;
        let wrapped_key = web3::signing::SecretKeyRef::new(eth_admin_key);
        let options = web3::contract::Options {
            gas: Some(PUSH_CERTIFICATE_GAS_LIMIT.into()),
            ..Default::default()
        };
        self.contract
            .signed_call_with_confirmations(
                "pushCertificate",
                (encoded_params, cert_position),
                options,
                1_usize,
                wrapped_key,
            )
            .await
            .map_err(|e| Error::Web3Error { source: e })
    }

    /// Ask subnet for latest pushed certificates, for every source subnet
    /// Returns list of latest stream positions for every source subnet
    pub async fn get_checkpoints(
        &self,
        target_subnet_id: &topos_core::uci::SubnetId,
    ) -> Result<Vec<TargetStreamPosition>, Error> {
        let op = || async {
            let mut target_stream_positions = Vec::new();

            let data: Vec<Token> = self
                .contract
                .query(
                    "getCheckpoints",
                    (),
                    None,
                    web3::contract::Options::default(),
                    None,
                )
                .await
                .map_err(|e| {
                    error!("Error retrieving checkpoints from smart contract: {e}");
                    Error::EthContractError { source: e }
                })?;

            for token in data {
                if let Token::Tuple(stream_position) = token {
                    let certificate_id = stream_position[0]
                        .clone()
                        .into_fixed_bytes()
                        .ok_or(Error::InvalidCheckpointsData)?;
                    let position = stream_position[1]
                        .clone()
                        .into_uint()
                        .ok_or(Error::InvalidCheckpointsData)?
                        .as_u64();
                    let source_subnet_id = stream_position[2]
                        .clone()
                        .into_fixed_bytes()
                        .ok_or(Error::InvalidCheckpointsData)?;

                    target_stream_positions.push(TargetStreamPosition {
                        target_subnet_id: *target_subnet_id,
                        certificate_id: Some(
                            TryInto::<[u8; 32]>::try_into(certificate_id)
                                .map_err(|_| Error::InvalidCheckpointsData)?
                                .into(),
                        ),
                        source_subnet_id: TryInto::<[u8; 32]>::try_into(source_subnet_id)
                            .map_err(|_| Error::InvalidCheckpointsData)?
                            .into(),
                        position,
                    });
                } else {
                    return Err(new_subnet_client_proxy_backoff_err(
                        Error::InvalidCheckpointsData,
                    ));
                }
            }

            Ok(target_stream_positions)
        };

        backoff::future::retry(backoff::ExponentialBackoff::default(), op).await
    }
}

// /// Create new backoff library error based on error that happened
pub(crate) fn new_subnet_client_proxy_backoff_err<E: std::fmt::Display>(
    err: E,
) -> backoff::Error<E> {
    // Retry according to backoff policy
    backoff::Error::Transient {
        err,
        retry_after: None,
    }
}

/// Create subnet client and open connection to the subnet
/// Retry until connection is valid
pub async fn connect_to_subnet_with_retry(
    http_subnet_endpoint: &str,
    eth_admin_private_key: Option<Vec<u8>>,
    contract_address: &str,
) -> Result<SubnetClient, crate::Error> {
    info!(
        "Connecting to subnet endpoint {} using backoff strategy...",
        http_subnet_endpoint
    );

    let op = || async {
        match SubnetClient::new(
            http_subnet_endpoint,
            eth_admin_private_key.clone(),
            contract_address,
        )
        .await
        {
            Ok(subnet_client) => Ok(subnet_client),
            Err(e) => {
                error!(
                    "Unable to instantiate http subnet client to endpoint {}: {e}",
                    http_subnet_endpoint,
                );
                Err(new_subnet_client_proxy_backoff_err(e))
            }
        }
    };

    backoff::future::retry(backoff::ExponentialBackoff::default(), op).await
}
