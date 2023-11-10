pub mod subnet_contract;

use crate::subnet_contract::{create_topos_core_contract_from_json, get_block_events};
use ethers::abi::ethabi::ethereum_types::{H160, U256};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::signers::Wallet;
use ethers::types::TransactionReceipt;
use ethers::{
    abi::Token,
    core::rand::thread_rng,
    middleware::SignerMiddleware,
    providers::{Http, Provider, ProviderError, StreamExt, Ws, WsClientError},
    signers::{LocalWallet, Signer, WalletError},
};
use ethers_providers::{Middleware, SubscriptionStream};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use topos_core::api::grpc::checkpoints::TargetStreamPosition;
pub use topos_core::uci::{
    Address, Certificate, CertificateId, ReceiptsRootHash, StateRoot, SubnetId, TxRootHash,
    CERTIFICATE_ID_LENGTH, SUBNET_ID_LENGTH,
};
use tracing::{error, info};

const PUSH_CERTIFICATE_GAS_LIMIT: u64 = 1000000;

pub type BlockData = Vec<u8>;
pub type BlockNumber = u64;
pub type Hash = String;

/// Event collected from the sending subnet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubnetEvent {
    CrossSubnetMessageSent { target_subnet_id: SubnetId },
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    /// hash of the block.
    pub hash: Hash,
    /// hash of the parent block.
    pub parent_hash: Hash,
    /// block's number.
    pub number: u64,
    /// state root
    pub state_root: StateRoot,
    /// tx root hash
    pub tx_root_hash: TxRootHash,
    /// receipts root hash
    pub receipts_root_hash: ReceiptsRootHash,
    /// Subnet events collected in this block
    pub events: Vec<SubnetEvent>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("new finalized block not available")]
    BlockNotAvailable(u64),
    #[error("next stream block not available")]
    StreamBlockNotAvailable,
    #[error("invalid block number: {0}")]
    InvalidBlockNumber(u64),
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
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("hex data decoding error: {0}")]
    HexDecodingError(rustc_hex::FromHexError),
    #[error("ethers provider error: {0}")]
    EthersProviderError(ProviderError),
    #[error("ethereum contract error: {0}")]
    ContractError(String),
    #[error("event decoding error: {0}")]
    EventDecodingError(String),
    #[error("ethereum event error: {0}")]
    EventError(String),
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },
    #[error("wallet error: {0}")]
    WalletError(WalletError),
    #[error("invalid secret key: {0}")]
    InvalidKey(String),
    #[error("error with signing ethereum transaction")]
    EthereumTxSignError,
    #[error("web socket client error: {0}")]
    WsClientError(WsClientError),
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
    contract: subnet_contract::IToposCore<Provider<Ws>>,
    provider: Arc<Provider<Ws>>,
}

impl SubnetClientListener {
    /// Initialize a new Subnet client
    pub async fn new(ws_subnet_endpoint: &str, contract_address: &str) -> Result<Self, Error> {
        info!(
            "Connecting to subnet node at endpoint: {}",
            ws_subnet_endpoint
        );
        let ws = Provider::<Ws>::connect(ws_subnet_endpoint)
            .await
            .map_err(Error::EthersProviderError)?;
        let provider = Arc::new(ws);

        // Initialize Topos Core Contract from json abi
        let contract = create_topos_core_contract_from_json(contract_address, provider.clone())?;

        Ok(SubnetClientListener { contract, provider })
    }

    pub async fn new_block_subscription_stream(
        &self,
    ) -> Result<SubscriptionStream<Ws, ethers::types::Block<ethers::types::H256>>, Error> {
        self.provider
            .subscribe_blocks()
            .await
            .map_err(Error::EthersProviderError)
    }

    /// Subscribe and listen to runtime finalized blocks
    pub async fn get_finalized_block(
        &mut self,
        next_block_number: u64,
    ) -> Result<BlockInfo, Error> {
        let latest_subnet_block_number = self
            .provider
            .get_block_number()
            .await
            .map_err(Error::EthersProviderError)?;

        info!(
            "Finalized block number: next={} and latest={}",
            next_block_number, latest_subnet_block_number
        );

        if latest_subnet_block_number.as_u64() < next_block_number {
            return Err(Error::BlockNotAvailable(next_block_number));
        }

        let block = self
            .provider
            .get_block(next_block_number)
            .await
            .map_err(Error::EthersProviderError)?
            .ok_or(Error::InvalidBlockNumber(next_block_number))?;
        let block_number = block
            .number
            .ok_or(Error::InvalidBlockNumber(next_block_number))?;
        let events = match get_block_events(&self.contract, block_number).await {
            Ok(events) => events,
            Err(Error::EventDecodingError(e)) => {
                // FIXME: Happens in block before subnet contract is deployed, seems like bug in ethers
                error!("Unable to parse events from block {}: {e}", block_number);
                Vec::new()
            }
            Err(e) => {
                error!("Unable to parse events from block {}: {e}", block_number);
                return Err(e);
            }
        };

        // Make block info result from all collected info
        let block_info = BlockInfo {
            hash: block.hash.unwrap_or_default().to_string(),
            parent_hash: block.parent_hash.to_string(),
            number: block_number.to_owned().as_u64(),
            state_root: block.state_root.0,
            tx_root_hash: block.transactions_root.0,
            receipts_root_hash: block.receipts_root.0,
            events,
        };
        info!(
            "Fetched new finalized block from subnet: {:?}",
            block_info.number
        );
        Ok(block_info)
    }

    /// Subscribe and listen to runtime finalized blocks
    pub async fn get_subnet_block_number(&mut self) -> Result<u64, Error> {
        self.provider
            .get_block_number()
            .await
            .map(|block_number| block_number.as_u64())
            .map_err(Error::EthersProviderError)
    }

    pub async fn wait_for_new_block(
        &self,
        stream: &mut SubscriptionStream<'_, Ws, ethers::types::Block<ethers::types::H256>>,
    ) -> Result<BlockInfo, Error> {
        if let Some(block) = stream.next().await {
            let block_number = block.number.ok_or(Error::DataNotAvailable)?;
            let events = match get_block_events(&self.contract, block_number).await {
                Ok(events) => events,
                Err(Error::EventDecodingError(e)) => {
                    // FIXME: Happens in block before subnet contract is deployed, seems like bug in ethers
                    error!("Unable to parse events from block {}: {e}", block_number);
                    Vec::new()
                }
                Err(e) => {
                    error!("Unable to parse events from block {}: {e}", block_number);
                    return Err(e);
                }
            };

            // Make block info result from all collected info
            let block_info = BlockInfo {
                hash: block.hash.unwrap_or_default().to_string(),
                parent_hash: block.parent_hash.to_string(),
                number: block_number.to_owned().as_u64(),
                state_root: block.state_root.0,
                tx_root_hash: block.transactions_root.0,
                receipts_root_hash: block.receipts_root.0,
                events,
            };
            Ok(block_info)
        } else {
            Err(Error::StreamBlockNotAvailable)
        }
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
    contract: subnet_contract::IToposCore<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
}

impl SubnetClient {
    /// Polling interval for event filters and pending transactions
    pub const NODE_POLLING_INTERVAL: Duration = Duration::from_millis(2000u64);

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

        let http = Provider::<Http>::try_from(http_subnet_endpoint)
            .map_err(|e| Error::InvalidUrl(e.to_string()))?
            .interval(SubnetClient::NODE_POLLING_INTERVAL);

        let wallet: LocalWallet = if let Some(eth_admin_secret_key) = &eth_admin_secret_key {
            hex::encode(eth_admin_secret_key)
                .parse()
                .map_err(Error::WalletError)?
        } else {
            // Dummy random key, will not be used to sign transactions
            LocalWallet::new(&mut thread_rng())
        };
        let chain_id = http
            .get_chainid()
            .await
            .map_err(Error::EthersProviderError)?;
        let client = Arc::new(SignerMiddleware::new(
            http,
            wallet.clone().with_chain_id(chain_id.as_u64()),
        ));
        // Initialize Topos Core Contract from json abi
        let contract = create_topos_core_contract_from_json(contract_address, client)?;

        let eth_admin_address = if let Some(eth_admin_secret_key) = eth_admin_secret_key {
            match subnet_contract::derive_eth_address(&eth_admin_secret_key) {
                Ok(address) => address,
                Err(e) => {
                    error!(
                        "Unable to derive admin address from secret key, error instantiating \
                         subnet client: {}",
                        e
                    );
                    return Err(e);
                }
            }
        } else {
            Default::default()
        };

        Ok(SubnetClient {
            eth_admin_address,
            contract,
        })
    }

    pub async fn push_certificate(
        &self,
        cert: &Certificate,
        cert_position: u64,
    ) -> Result<Option<TransactionReceipt>, Error> {
        let prev_cert_id: Token = Token::FixedBytes(cert.prev_id.as_array().to_vec());
        let source_subnet_id: Token = Token::FixedBytes(cert.source_subnet_id.into());
        let state_root: Token = Token::FixedBytes(cert.state_root.to_vec());
        let tx_root: Token = Token::FixedBytes(cert.tx_root_hash.to_vec());
        let receipt_root: Token = Token::FixedBytes(cert.receipts_root_hash.to_vec());
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
        let cert_position = U256::from(cert_position);
        let encoded_cert_bytes = ethers::abi::encode(&[
            prev_cert_id,
            source_subnet_id,
            state_root,
            tx_root,
            receipt_root,
            target_subnets,
            verifier,
            cert_id,
            stark_proof,
            signature,
        ]);

        let tx = self
            .contract
            .push_certificate(encoded_cert_bytes.into(), cert_position)
            .gas(PUSH_CERTIFICATE_GAS_LIMIT)
            .legacy(); // Polygon Edge only supports legacy transactions

        let receipt = tx
            .send()
            .await
            .map_err(|e| {
                error!("Unable to push certificate: {e}");
                Error::ContractError(e.to_string())
            })?
            .await
            .map_err(Error::EthersProviderError)?;

        Ok(receipt)
    }

    /// Ask subnet for latest pushed certificates, for every source subnet
    /// Returns list of latest stream positions for every source subnet
    pub async fn get_checkpoints(
        &self,
        target_subnet_id: &topos_core::uci::SubnetId,
    ) -> Result<Vec<TargetStreamPosition>, Error> {
        let op = || async {
            let mut target_stream_positions: Vec<TargetStreamPosition> = Vec::new();
            let stream_positions = self.contract.get_checkpoints().call().await.map_err(|e| {
                error!("Unable to get checkpoints: {e}");
                Error::ContractError(e.to_string())
            })?;

            for position in stream_positions {
                target_stream_positions.push(TargetStreamPosition {
                    target_subnet_id: *target_subnet_id,
                    certificate_id: Some(
                        TryInto::<[u8; CERTIFICATE_ID_LENGTH]>::try_into(position.cert_id)
                            .map_err(|_| Error::InvalidCheckpointsData)?
                            .into(),
                    ),
                    source_subnet_id: TryInto::<[u8; SUBNET_ID_LENGTH]>::try_into(
                        position.source_subnet_id,
                    )
                    .map_err(|_| Error::InvalidCheckpointsData)?
                    .into(),
                    position: position.position.as_u64(),
                });
            }

            Ok(target_stream_positions)
        };

        backoff::future::retry(backoff::ExponentialBackoff::default(), op).await
    }

    /// Ask subnet for its subnet id
    pub async fn get_subnet_id(&self) -> Result<SubnetId, Error> {
        let op = || async {
            let subnet_id = self
                .contract
                .network_subnet_id()
                .call()
                .await
                .map_err(|e| {
                    error!("Unable to query network subnet id: {e}");
                    Error::ContractError(e.to_string())
                })?;
            Ok(SubnetId::from_array(subnet_id))
        };

        backoff::future::retry(backoff::ExponentialBackoff::default(), op).await
    }
}

/// Create new backoff library error based on error that happened
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
    signing_key: Option<Vec<u8>>,
    contract_address: &str,
) -> Result<SubnetClient, crate::Error> {
    info!(
        "Connecting to subnet endpoint {} using backoff strategy...",
        http_subnet_endpoint
    );

    let op = || async {
        match SubnetClient::new(http_subnet_endpoint, signing_key.clone(), contract_address).await {
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
