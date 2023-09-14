//! implementation of Topos Network Transport
//!
use clap::Parser;
use ethers::prelude::{SignatureError, Signer, WalletError};
use ethers::signers::LocalWallet;
use ethers::types::{Signature, H160};
use ethers::utils::keccak256;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use topos_core::uci::{Certificate, CertificateId};
use topos_p2p::PeerId;
use tracing::error;

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

    /// Certificate successfully delivered
    CertificateDelivered {
        certificate: Certificate,
    },

    /// Stable Sample
    StableSample,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ValidatorId(H160);

impl ValidatorId {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl From<H160> for ValidatorId {
    fn from(address: H160) -> Self {
        ValidatorId(address)
    }
}

impl From<&str> for ValidatorId {
    fn from(address: &str) -> Self {
        let h160 = H160::from_str(address).expect("Failed to parse address");
        ValidatorId(h160)
    }
}

impl std::fmt::Display for ValidatorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

pub async fn sign_message(
    validator_id: ValidatorId,
    certificate_id: CertificateId,
    wallet: LocalWallet,
) -> Result<Signature, WalletError> {
    let mut hash = Vec::new();
    hash.extend(certificate_id.as_array().iter().cloned());
    hash.extend(validator_id.as_bytes());

    let hash = keccak256(hash);
    error!("SIGN: hash: {hash:?} val_id: {validator_id} cert_id: {certificate_id}");
    wallet.sign_message(hash).await
}

pub fn verify_signature(
    signature: Signature,
    validator_id: ValidatorId,
    certificate_id: CertificateId,
    wallet: LocalWallet,
) -> Result<(), SignatureError> {
    let public_key = wallet.address();
    let mut hash = Vec::new();
    hash.extend(certificate_id.as_array().iter().cloned());
    hash.extend(validator_id.as_bytes());

    let hash = keccak256(hash);
    error!("VERIFY: hash: {hash:?} val_id: {validator_id} cert_id: {certificate_id} public_key: {public_key}");
    signature.verify(hash, public_key)
}
