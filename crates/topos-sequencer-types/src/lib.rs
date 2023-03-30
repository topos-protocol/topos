//! Implementation of Topos Network Transport
//!
use opentelemetry::Context;
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

#[derive(Debug, Clone)]
pub enum SubnetRuntimeProxyEvent {
    // New certificate is generated
    NewCertificate {
        cert: Box<Certificate>,
        ctx: Context,
    },
    /// New set of authorities in charge of the threshold signature
    NewEra(Vec<Authorities>),
}

#[derive(Debug)]
pub enum SubnetRuntimeProxyCommand {
    /// Upon receiving a new delivered Certificate from the TCE
    OnNewDeliveredCertificate {
        certificate: Certificate,
        position: u64,
        ctx: Context,
    },
}

#[derive(Debug)]
pub enum TceProxyCommand {
    /// Submit a newly created certificate to the TCE
    SubmitCertificate {
        cert: Box<Certificate>,
        ctx: Context,
    },

    /// Shutdown command
    Shutdown(tokio::sync::oneshot::Sender<()>),
}

#[derive(Debug, Clone)]
pub enum TceProxyEvent {
    /// New delivered certificate (and its position) fetched from the TCE network
    NewDeliveredCerts {
        certificates: Vec<(Certificate, u64)>,
        ctx: Context,
    },
    /// Failed watching certificates channel
    /// Requires restart of sequencer tce proxy
    WatchCertificatesChannelFailed,
}

// A wrapper to handle all events
#[derive(Debug, Clone)]
pub enum Event {
    RuntimeProxyEvent(SubnetRuntimeProxyEvent),
}
