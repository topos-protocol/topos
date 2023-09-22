//! implementation of Topos Network Transport
//!
use clap::Parser;
use ethers::types::{Address, Signature, H160};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;
use topos_core::uci::{Certificate, CertificateId};
use topos_p2p::PeerId;

#[derive(Parser, Clone, Debug, Default, Deserialize, Serialize)]
#[command(name = "Parameters of the reliable broadcast")]
pub struct ReliableBroadcastParams {
    /// Echo threshold
    #[arg(long, env = "TCE_ECHO_THRESHOLD", default_value_t = 1)]
    pub echo_threshold: usize,
    /// Ready threshold
    #[arg(long, env = "TCE_READY_THRESHOLD", default_value_t = 1)]
    pub ready_threshold: usize,
    /// Delivery threshold
    #[arg(long, env = "TCE_DELIVERY_THRESHOLD", default_value_t = 1)]
    pub delivery_threshold: usize,
}

impl ReliableBroadcastParams {
    pub fn new(n: usize) -> Self {
        let f: usize = n / 3;

        Self {
            echo_threshold: 1 + (n + f) / 2,
            ready_threshold: 1 + f,
            delivery_threshold: 2 * f + 1,
        }
    }
}

/// Protocol commands
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TceCommands {
    /// Initialize the instance, signals the environment is ready
    StartUp,
    /// Shuts down the instance
    Shutdown,
    /// Entry point for new certificate to submit as initial sender
    OnBroadcast { cert: Certificate },
    /// We got updated list of visible peers to work with, let protocol do the sampling
    OnVisiblePeersChanged { peers: Vec<PeerId> },
    /// Given peer sent EchoSubscribe request
    OnEchoSubscribeReq {},
    /// Given peer sent ReadySubscribe request
    OnReadySubscribeReq {},
    /// Given peer replied ok to the EchoSubscribe request
    OnEchoSubscribeOk {},
    /// Given peer replied ok to the ReadySubscribe request
    OnReadySubscribeOk {},
    /// Upon new certificate to start delivery
    OnStartDelivery { cert: Certificate },
    /// Received G-set message
    OnGossip { cert: Certificate },
    /// When echo reply received
    OnEcho {
        certificate_id: CertificateId,
        signature: Signature,
        validator_id: ValidatorId,
    },
    /// When ready reply received
    OnReady {
        certificate_id: CertificateId,
        signature: Signature,
        validator_id: ValidatorId,
    },
    /// Given peer replied ok to the double echo request
    OnDoubleEchoOk {},
}

/// Protocol events
#[derive(Clone, Debug)]
pub enum ProtocolEvents {
    BroadcastFailed {
        certificate_id: CertificateId,
    },
    AlreadyDelivered {
        certificate_id: CertificateId,
    },
    /// Emitted to get peers list, expected that Commands.ApplyPeers will come as reaction
    NeedPeers,
    /// (pb.Broadcast)
    Broadcast {
        certificate_id: CertificateId,
    },
    /// After sampling is done we ask peers to participate in the protocol (and provide us echo feedback)
    EchoSubscribeReq {
        peers: Vec<PeerId>,
    },
    /// After sampling is done we ask peers to participate in the protocol
    /// (and provide us ready/delivery feedback (both with Ready message))
    ReadySubscribeReq {
        peers: Vec<PeerId>,
    },
    /// We are ok to participate in the protocol and confirm that to subscriber
    EchoSubscribeOk {
        to_peer: PeerId,
    },
    /// We are ok to participate in the protocol and confirm that to subscriber
    ReadySubscribeOk {
        to_peer: PeerId,
    },
    /// Indicates that 'gossip' message broadcasting is required
    Gossip {
        cert: Certificate,
    },
    /// Indicates that 'echo' message broadcasting is required
    Echo {
        certificate_id: CertificateId,
        signature: Signature,
        validator_id: ValidatorId,
    },
    /// Indicates that 'ready' message broadcasting is required
    Ready {
        certificate_id: CertificateId,
        signature: Signature,
        validator_id: ValidatorId,
    },
    /// For simulation purpose, for now only caused by ill-formed sampling
    Die,

    /// Stable Sample
    StableSample,
}

#[derive(Debug, Error)]
pub enum ValidatorIdConversionError {
    #[error("Failed to parse address string as H160")]
    ParseError,
    #[error("Failed to convert byte array into H160")]
    InvalidByteLength,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ValidatorId(H160);

impl ValidatorId {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn address(&self) -> Address {
        self.0
    }
}

impl From<H160> for ValidatorId {
    fn from(address: H160) -> Self {
        ValidatorId(address)
    }
}

impl TryFrom<&str> for ValidatorId {
    type Error = ValidatorIdConversionError;

    fn try_from(address: &str) -> Result<Self, Self::Error> {
        H160::from_str(address)
            .map_err(|_| ValidatorIdConversionError::ParseError)
            .map(ValidatorId)
    }
}

impl std::fmt::Display for ValidatorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}
