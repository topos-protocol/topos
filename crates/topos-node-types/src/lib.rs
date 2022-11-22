//! Implementation of Topos Network Transport
//!
use serde::{Deserialize, Serialize};
pub use topos_core::uci::{
    Certificate, CrossChainTransaction, CrossChainTransactionData, DigestCompressed,
};

// TODO: proper type definitions
pub type BlockData = Vec<u8>;
pub type BlockNumber = u32;
pub type Hash = String;

/// Event collected from the sending subnet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubnetEvent {
    SendToken {
        terminal_subnet_id: String,
        asset_id: sp_core::U256,
        sender_addr: String,
        recipient_addr: String,
        amount: sp_core::U256,
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
    /// Block's extrinsics.
    pub data: BlockData,
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

#[derive(Debug, Serialize, Deserialize)]
pub enum CertificationCommand {
    /// Instruction update list of finalized blocks
    AddFinalizedBlock(BlockInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeProxyEvent {
    /// New Finalized block
    BlockFinalized(BlockInfo),
    /// New set of authorities in charge of threshold signature
    NewEra(Vec<Authorities>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum RuntimeProxyCommand {
    /// Push the Certificate to the subnet runtime
    PushCertificate(Certificate),

    /// propagate transacitons to the runtime
    OnNewDeliveredTxns(Certificate),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TceProxyCommand {
    /// Submit a newly created certificate to the TCE network
    SubmitCertificate(Certificate),
    /// Exit command
    Exit,
}

#[derive(Debug, Clone)]
pub enum TceProxyEvent {
    /// New delivered certificate fetched from the TCE network
    NewDeliveredCerts(Vec<Certificate>),
}

// A wrapper to handle all events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    CertificationEvent(CertificationEvent),
    RuntimeProxyEvent(RuntimeProxyEvent),
}

/// Protocol commands
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TrbpCommands {
    /// Initialize the instance, signals the environment is ready
    StartUp,
    /// Shuts down the instance
    Shutdown,
    /// Entry point for new certificate to submit as initial sender
    OnBroadcast { cert: Certificate },
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
        cert: Certificate,
        digest: DigestCompressed,
    },
    /// Received G-set message
    OnGossip {
        cert: Certificate,
        digest: DigestCompressed,
    },
    /// When echo reply received
    OnEcho {
        from_peer: String,
        cert: Certificate,
    },
    /// When ready reply received
    OnReady {
        from_peer: String,
        cert: Certificate,
    },
}

/// Protocol events
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrbpEvents {
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
