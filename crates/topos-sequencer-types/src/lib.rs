//! Implementation of Topos Network Transport
//!

use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use topos_core::uci::{Address, Certificate, CertificateId, StateRoot, SubnetId, TxRootHash};

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

    /// Propagate certificate and its position to the runtime
    OnNewDeliveredTxns((Certificate, u64)),
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
    /// New delivered certificate (and its position) fetched from the TCE network
    NewDeliveredCerts(Vec<(Certificate, u64)>),
    /// Failed watching certificates channel
    /// Requires restart of sequencer tce proxy
    WatchCertificatesChannelFailed,
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
    /// Received G-set message
    OnGossip { cert: Box<Certificate> },
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
