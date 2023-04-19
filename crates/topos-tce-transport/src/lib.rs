//! implementation of Topos Network Transport
//!
use clap::Args;
use serde::{Deserialize, Serialize};
use topos_core::uci::{Certificate, CertificateId};
use topos_p2p::PeerId;
use topos_telemetry::PropagationContext;
use tracing::Span;

#[derive(Args, Default, Clone, Debug)]
#[command(name = "Parameters of the reliable broadcast")]
pub struct ReliableBroadcastParams {
    /// Echo threshold
    #[arg(long, default_value_t = 1, env = "TCE_ECHO_THRESHOLD")]
    pub echo_threshold: usize,
    /// Ready threshold
    #[arg(long, default_value_t = 1, env = "TCE_READY_THRESHOLD")]
    pub ready_threshold: usize,
    /// Delivery threshold
    #[arg(long, default_value_t = 1, env = "TCE_DELIVERY_THRESHOLD")]
    pub delivery_threshold: usize,
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
    OnEchoSubscribeReq { from_peer: PeerId },
    /// Given peer sent ReadySubscribe request
    OnReadySubscribeReq { from_peer: PeerId },
    /// Given peer replied ok to the EchoSubscribe request
    OnEchoSubscribeOk { from_peer: PeerId },
    /// Given peer replied ok to the ReadySubscribe request
    OnReadySubscribeOk { from_peer: PeerId },
    /// Upon new certificate to start delivery
    OnStartDelivery { cert: Certificate },
    /// Received G-set message
    OnGossip {
        cert: Certificate,
        ctx: PropagationContext,
        // root_ctx: PropagationContext,
    },
    /// When echo reply received
    OnEcho {
        from_peer: PeerId,
        certificate_id: CertificateId,
        ctx: PropagationContext,
        // root_ctx: PropagationContext,
    },
    /// When ready reply received
    OnReady {
        from_peer: PeerId,
        certificate_id: CertificateId,
        ctx: PropagationContext,
    },
    /// Given peer replied ok to the double echo request
    OnDoubleEchoOk { from_peer: PeerId },
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
    UnableToBufferCertificate {
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
        peers: Vec<PeerId>,
        cert: Certificate,
        ctx: Span,
        // root_ctx: PropagationContext,
    },
    /// Indicates that 'echo' message broadcasting is required
    Echo {
        peers: Vec<PeerId>,
        certificate_id: CertificateId,
        ctx: Span,
        // root_ctx: PropagationContext,
    },
    /// Indicates that 'ready' message broadcasting is required
    Ready {
        peers: Vec<PeerId>,
        certificate_id: CertificateId,
        ctx: Span,
    },
    /// For simulation purpose, for now only caused by ill-formed sampling
    Die,

    /// Certificate successfully delivered
    CertificateDelivered {
        certificate: Certificate,
    },

    /// Stable Sample
    StableSample(Vec<PeerId>),
}
