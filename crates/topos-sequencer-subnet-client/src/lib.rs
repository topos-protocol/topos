pub mod subnet_contract;

use crate::subnet_contract::{
    create_topos_core_contract_from_json, parse_events_from_json, parse_events_from_log,
};
use thiserror::Error;
use topos_sequencer_types::BlockInfo;
use tracing::{debug, error, info};
use web3::futures::StreamExt;
use web3::transports::WebSocket;
use web3::types::{BlockId, BlockNumber, U64};

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
}

// A to hold subnet client related entities
pub struct SubnetClient {
    pub eth_admin_address: String,
    #[allow(dead_code)]
    eth_admin_key: Vec<u8>,
    web3_client: web3::Web3<web3::transports::WebSocket>,
    latest_block: Option<u64>,
    block_subscription: Option<
        web3::api::SubscriptionStream<web3::transports::WebSocket, web3::types::BlockHeader>,
    >,
    #[allow(dead_code)]
    contract: web3::contract::Contract<WebSocket>,
    events: Vec<web3::ethabi::Event>,
}

impl SubnetClient {
    /// Initialize a new Subnet client
    pub async fn new(
        subnet_endpoint: &str,
        eth_admin_key: Vec<u8>,
        contract_address: &str,
    ) -> Result<Self, Error> {
        info!("Connecting to subnet node at endpoint: {}", subnet_endpoint);
        let transport = web3::transports::WebSocket::new(subnet_endpoint).await?;
        let web3 = web3::Web3::new(transport);
        let eth_admin_address = match subnet_contract::derive_eth_address(&eth_admin_key) {
            Ok(address) => address,
            Err(e) => {
                error!("Unable to derive admin addres from secret key, error instantiating subnet client: {}", e);
                return Err(e);
            }
        };

        // Initialize Topos Core Contract from json abi
        let contract = create_topos_core_contract_from_json(&web3, contract_address)?;
        // List of events that this contract could create
        let events = parse_events_from_json()?;

        Ok(SubnetClient {
            eth_admin_key,
            eth_admin_address,
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

    pub async fn get_eth_nonce(&self, _address: &str) -> Result<u128, Error> {
        // Get transaction count from native blockchain for particular account
        // TODO implement with web3 client
        Ok(0)
    }
}
