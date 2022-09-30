//! implementation of Topos Network Transport
//!
use clap::Parser;
use serde::{Deserialize, Serialize};
use topos_core::uci::Certificate;

/// Protocol parameters of the TRB
#[derive(Default, Clone, Debug, Parser)]
#[clap(name = "Protocol parameters of the TRB")]
pub struct ReliableBroadcastParams {
    /// Echo threshold
    #[clap(long, default_value_t = 1, env = "TCE_TRBP_ECHO_THRESHOLD")]
    pub echo_threshold: usize,
    /// Echo sample size
    #[clap(long, default_value_t = 1, env = "TCE_TRBP_ECHO_SAMPLE_SIZE")]
    pub echo_sample_size: usize,
    /// Ready threshold
    #[clap(long, default_value_t = 1, env = "TCE_TRBP_READY_THRESHOLD")]
    pub ready_threshold: usize,
    /// Ready sample size
    #[clap(long, default_value_t = 1, env = "TCE_TRBP_READY_SAMPLE_SIZE")]
    pub ready_sample_size: usize,
    /// Delivery threshold
    #[clap(long, default_value_t = 1, env = "TCE_TRBP_DELIVERY_THRESHOLD")]
    pub delivery_threshold: usize,
    /// Delivery sample size
    #[clap(long, default_value_t = 1, env = "TCE_TRBP_DELIVERY_SAMPLE_SIZE")]
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
    OnVisiblePeersChanged { peers: Vec<String> },
    /// Given peer sent EchoSubscribe request
    OnEchoSubscribeReq { from_peer: String },
    /// Given peer sent ReadySubscribe request
    OnReadySubscribeReq { from_peer: String },
    /// Given peer replied ok to the EchoSubscribe request
    OnEchoSubscribeOk { from_peer: String },
    /// Given peer replied ok to the ReadySubscribe request
    OnReadySubscribeOk { from_peer: String },
    /// Upon new certificate to start delivery
    OnStartDelivery { cert: Certificate },
    /// Received G-set message
    OnGossip { cert: Certificate },
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
    /// Given peer replied ok to the double echo request
    OnDoubleEchoOk { from_peer: String },
}

/// Protocol events
#[derive(Clone, Debug)]
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

    /// Certificate successfully delivered
    CertificateDelivered { certificate: Certificate },
}
