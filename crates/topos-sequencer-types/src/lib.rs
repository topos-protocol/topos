//! Implementation of Topos Network Transport
//!
#![feature(iterator_try_collect)]
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use topos_core::uci::{
    Address, Certificate, CertificateId, DigestCompressed, StateRoot, SubnetId, TxRootHash,
};

pub type BlockData = Vec<u8>;
pub type BlockNumber = u64;
pub type Hash = String;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid data conversion: {0}")]
    InvalidDataConversion(String),
}

/// Event collected from the sending subnet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubnetEvent {
    TokenSent {
        sender: Address,
        source_subnet_id: SubnetId,
        target_subnet_id: SubnetId,
        receiver: Address,
        symbol: String,
        amount: ethereum_types::U256,
    },
    ContractCall {
        source_subnet_id: SubnetId,
        source_contract_addr: Address,
        target_subnet_id: SubnetId,
        target_contract_addr: Address,
        payload: Vec<u8>,
    },
    ContractCallWithToken {
        source_subnet_id: SubnetId,
        source_contract_addr: Address,
        target_subnet_id: SubnetId,
        target_contract_addr: Address,
        payload: Vec<u8>,
        symbol: String,
        amount: ethereum_types::U256,
    },
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    // TODO: proper dependencies to block type etc
    /// hash of the block.
    pub hash: Hash,
    /// hash of the parent block.
    pub parent_hash: Hash,
    /// block's number.
    pub number: BlockNumber,
    /// state root
    pub state_root: StateRoot,
    /// tx root hash
    pub tx_root_hash: TxRootHash,
    /// Subnet events collected in this block
    pub events: Vec<SubnetEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authorities {
    // TODO: proper dependencies to block type etc
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CertificationEvent {
    /// Created Certificate ready to be threshold signed
    NewCertificate(Certificate),
}

#[derive(Debug)]
pub enum CertificationCommand {
    /// Instruction update list of finalized blocks
    AddFinalizedBlock(BlockInfo),
}

#[derive(Debug, Clone)]
pub enum SubnetRuntimeProxyEvent {
    /// New Finalized block
    BlockFinalized(BlockInfo),
    /// New set of authorities in charge of threshold signature
    NewEra(Vec<Authorities>),
}

#[derive(Debug)]
pub enum SubnetRuntimeProxyCommand {
    /// Push the Certificate to the subnet runtime
    PushCertificate(Certificate),

    /// propagate transacitons to the runtime
    OnNewDeliveredTxns(Certificate),
}

#[derive(Debug)]
pub enum TceProxyCommand {
    /// Submit a newly created certificate to the TCE network
    SubmitCertificate(Box<Certificate>),

    /// Shutdown command
    Shutdown(tokio::sync::oneshot::Sender<()>),
}

#[derive(Debug, Clone)]
pub enum TceProxyEvent {
    /// New delivered certificate fetched from the TCE network
    NewDeliveredCerts(Vec<Certificate>),
}

// A wrapper to handle all events
#[derive(Debug, Clone)]
pub enum Event {
    CertificationEvent(CertificationEvent),
    RuntimeProxyEvent(SubnetRuntimeProxyEvent),
}

/// Protocol commands
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TceCommands {
    /// Initialize the instance, signals the environment is ready
    StartUp,
    /// Shuts down the instance
    Shutdown,
    /// Entry point for new certificate to submit as initial sender
    OnBroadcast { cert: Box<Certificate> },
    /// We got updated list of visible peers to work with, let protocol do the sampling
    OnVisiblePeersChanged { peers: Vec<String> },
    /// We got updated list of connected peers to gossip to
    OnConnectedPeersChanged { peers: Vec<String> },
    /// Given peer sent EchoSubscribe request
    OnEchoSubscribeReq { from_peer: String },
    /// Given peer sent ReadySubscribe request
    OnReadySubscribeReq { from_peer: String },
    /// Given peer replied ok to the EchoSubscribe request
    OnEchoSubscribeOk { from_peer: String },
    /// Given peer replied ok to the ReadySubscribe request
    OnReadySubscribeOk { from_peer: String },
    /// Upon new certificate to start delivery
    OnStartDelivery {
        cert: Box<Certificate>,
        digest: DigestCompressed,
    },
    /// Received G-set message
    OnGossip {
        cert: Box<Certificate>,
        digest: DigestCompressed,
    },
    /// When echo reply received
    OnEcho {
        from_peer: String,
        cert: Box<Certificate>,
    },
    /// When ready reply received
    OnReady {
        from_peer: String,
        cert: Box<Certificate>,
    },
}

/// Protocol events
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TceEvents {
    /// Emitted to get peers list, expected that Commands.ApplyPeers will come as reaction
    NeedPeers,
    /// (pb.Broadcast)
    Broadcast { cert: Certificate },
    /// After sampling is done we ask peers to participate in the protocol
    EchoSubscribeReq { peers: Vec<String> },
    /// After sampling is done we ask peers to participate in the protocol
    ReadySubscribeReq { peers: Vec<String> },
    /// We are ok to participate in the protocol
    EchoSubscribeOk { to_peer: String },
    /// We are ok to participate in the protocol
    ReadySubscribeOk { to_peer: String },
    /// Indicates that 'gossip' message broadcasting is required
    Gossip {
        peers: Vec<String>,
        cert: Certificate,
        digest: DigestCompressed,
    },
    /// Indicates that 'echo' message broadcasting is required
    Echo {
        peers: Vec<String>,
        cert: Certificate,
    },
    /// Indicates that 'ready' message broadcasting is required
    Ready {
        peers: Vec<String>,
        cert: Certificate,
    },
    /// For simulation purpose, for now only caused by ill-formed sampling
    Die,
}

pub struct TargetStreamPosition {
    pub source_subnet_id: SubnetId,
    pub target_subnet_id: SubnetId,
    pub position: u64,
    pub certificate_id: CertificateId,
}

impl TryFrom<topos_core::api::shared::v1::positions::TargetStreamPosition>
    for TargetStreamPosition
{
    type Error = Error;
    fn try_from(
        value: topos_core::api::shared::v1::positions::TargetStreamPosition,
    ) -> Result<Self, Error> {
        // Target stream position is retrieved from TCE node who is trusted entity
        // If it sends invalid data, panic
        Ok(Self {
            target_subnet_id: value
                .target_subnet_id
                .ok_or(Error::InvalidDataConversion(
                    "Invalid target subnet id".to_string(),
                ))?
                .try_into()
                .map_err(|_: <[u8; 32] as TryFrom<Vec<u8>>>::Error| {
                    Error::InvalidDataConversion("Invalid target subnet id".to_string())
                })?,
            source_subnet_id: value
                .source_subnet_id
                .ok_or(Error::InvalidDataConversion(
                    "Invalid source subnet id".to_string(),
                ))?
                .try_into()
                .map_err(|_: <[u8; 32] as TryFrom<Vec<u8>>>::Error| {
                    Error::InvalidDataConversion("Invalid source subnet id".to_string())
                })?,
            certificate_id: value
                .certificate_id
                .ok_or(Error::InvalidDataConversion(
                    "Invalid certificate id".to_string(),
                ))?
                .value
                .try_into()
                .map_err(|e: topos_core::uci::Error| Error::InvalidDataConversion(e.to_string()))?,
            position: value.position,
        })
    }
}

impl From<TargetStreamPosition> for topos_core::api::shared::v1::positions::TargetStreamPosition {
    fn from(value: TargetStreamPosition) -> Self {
        Self {
            target_subnet_id: Some(value.target_subnet_id.into()),
            source_subnet_id: Some(value.source_subnet_id.into()),
            certificate_id: Some((*value.certificate_id.as_array()).into()),
            position: value.position,
        }
    }
}

pub struct TargetCheckpoint {
    pub target_subnet_ids: Vec<SubnetId>,
    pub positions: Vec<TargetStreamPosition>,
}

impl TryFrom<topos_core::api::shared::v1::checkpoints::TargetCheckpoint> for TargetCheckpoint {
    type Error = Error;
    fn try_from(
        value: topos_core::api::shared::v1::checkpoints::TargetCheckpoint,
    ) -> Result<Self, Error> {
        Ok(Self {
            target_subnet_ids: value
                .target_subnet_ids
                .into_iter()
                .map(|c| c.value.try_into())
                .try_collect()
                .map_err(|_: <[u8; 32] as TryFrom<Vec<u8>>>::Error| {
                    Error::InvalidDataConversion("Invalid target subnet id".to_string())
                })?,
            positions: value
                .positions
                .into_iter()
                .map(|p| p.try_into())
                .try_collect()?,
        })
    }
}

impl From<TargetCheckpoint> for topos_core::api::shared::v1::checkpoints::TargetCheckpoint {
    fn from(value: TargetCheckpoint) -> Self {
        Self {
            target_subnet_ids: value
                .target_subnet_ids
                .into_iter()
                .map(|c| c.into())
                .collect(),
            positions: value.positions.into_iter().map(|p| p.into()).collect(),
        }
    }
}
