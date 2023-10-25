//! implementation of Topos Network Transport
//!
use clap::Parser;
use serde::{Deserialize, Serialize};
use topos_core::{
    types::ValidatorId,
    uci::{Certificate, CertificateId},
};
use topos_crypto::messages::Signature;

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

// /// Protocol commands
// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub enum TceCommands {
//     /// Received G-set message
//     OnGossip { cert: Certificate },
//     /// When echo reply received
//     OnEcho {
//         certificate_id: CertificateId,
//         signature: Signature,
//         validator_id: ValidatorId,
//     },
//     /// When ready reply received
//     OnReady {
//         certificate_id: CertificateId,
//         signature: Signature,
//         validator_id: ValidatorId,
//     },
// }

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
