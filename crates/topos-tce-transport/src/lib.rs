//! implementation of Topos Network Transport
//!
use clap::Args;
use serde::{Deserialize, Serialize};
use topos_core::uci::{Certificate, DigestCompressed};
use topos_p2p::PeerId;

/// Protocol parameters of the TRB
#[derive(Args, Default, Clone, Debug)]
#[command(name = "Protocol parameters of the TRB")]
pub struct ReliableBroadcastParams {
    /// Echo threshold
    #[arg(long, default_value_t = 1, env = "TCE_TRBP_ECHO_THRESHOLD")]
    pub echo_threshold: usize,
    /// Echo sample size
    #[arg(long, default_value_t = 1, env = "TCE_TRBP_ECHO_SAMPLE_SIZE")]
    pub echo_sample_size: usize,
    /// Ready threshold
    #[arg(long, default_value_t = 1, env = "TCE_TRBP_READY_THRESHOLD")]
    pub ready_threshold: usize,
    /// Ready sample size
    #[arg(long, default_value_t = 1, env = "TCE_TRBP_READY_SAMPLE_SIZE")]
    pub ready_sample_size: usize,
    /// Delivery threshold
    #[arg(long, default_value_t = 1, env = "TCE_TRBP_DELIVERY_THRESHOLD")]
    pub delivery_threshold: usize,
    /// Delivery sample size
    #[arg(long, default_value_t = 1, env = "TCE_TRBP_DELIVERY_SAMPLE_SIZE")]
    pub delivery_sample_size: usize,
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
        from_peer: PeerId,
        cert: Certificate,
    },
    /// When ready reply received
    OnReady {
        from_peer: PeerId,
        cert: Certificate,
    },
    /// Given peer replied ok to the double echo request
    OnDoubleEchoOk { from_peer: PeerId },
}

/// Protocol events
#[derive(Clone, Debug)]
pub enum TrbpEvents {
    /// Emitted to get peers list, expected that Commands.ApplyPeers will come as reaction
    NeedPeers,
    /// (pb.Broadcast)
    Broadcast { cert: Certificate },
    /// After sampling is done we ask peers to participate in the protocol (and provide us echo feedback)
    EchoSubscribeReq { peers: Vec<PeerId> },
    /// After sampling is done we ask peers to participate in the protocol
    /// (and provide us ready/delivery feedback (both with Ready message))
    ReadySubscribeReq { peers: Vec<PeerId> },
    /// We are ok to participate in the protocol and confirm that to subscriber
    EchoSubscribeOk { to_peer: PeerId },
    /// We are ok to participate in the protocol and confirm that to subscriber
    ReadySubscribeOk { to_peer: PeerId },
    /// Indicates that 'gossip' message broadcasting is required
    Gossip {
        peers: Vec<PeerId>,
        cert: Certificate,
        digest: DigestCompressed,
    },
    /// Indicates that 'echo' message broadcasting is required
    Echo {
        peers: Vec<PeerId>,
        cert: Certificate,
    },
    /// Indicates that 'ready' message broadcasting is required
    Ready {
        peers: Vec<PeerId>,
        cert: Certificate,
    },
    /// For simulation purpose, for now only caused by ill-formed sampling
    Die,

    /// Certificate successfully delivered
    CertificateDelivered { certificate: Certificate },
}
