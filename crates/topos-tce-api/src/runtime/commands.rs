use std::collections::HashMap;

use opentelemetry::Context;
use tokio::sync::{mpsc::Sender, oneshot};
use topos_core::uci::{Certificate, SubnetId};
use topos_p2p::PeerId;
use topos_tce_types::checkpoints::TargetStreamPosition;
use uuid::Uuid;

use crate::stream::{Stream, StreamCommand};

use super::error::RuntimeError;

#[derive(Debug)]
pub enum RuntimeCommand {
    /// This command is dispatch when a certificate is ready to be dispatch to related subnet
    DispatchCertificate { certificate: Certificate },
}

#[derive(Debug)]
pub(crate) enum InternalRuntimeCommand {
    /// When a new stream is open, this command is dispatch to manage the stream
    NewStream {
        stream: Stream,
        command_sender: Sender<StreamCommand>,
    },

    /// Register a stream as subscriber for the given subnet_streams.
    /// Commands or certificates pointing to one of the subnet will be forward using the given Sender
    Register {
        stream_id: Uuid,
        #[allow(dead_code)]
        target_subnet_stream_positions: HashMap<SubnetId, HashMap<SubnetId, TargetStreamPosition>>,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },

    /// Notify that a Stream has successfully handshake with the server
    Handshaked { stream_id: Uuid },

    /// Dispatch when a certificate has been submitted to the TCE.
    /// This command will be used to trigger the DoubleEcho process.
    CertificateSubmitted {
        certificate: Box<Certificate>,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
        ctx: Context,
    },

    /// Push a new list of PeerId to be used by the Gatekeeper
    PushPeerList {
        peers: Vec<PeerId>,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },

    /// Get source head certificate by source subnet id
    GetSourceHead {
        subnet_id: topos_core::uci::SubnetId,
        sender: oneshot::Sender<Result<(u64, topos_core::uci::Certificate), RuntimeError>>,
    },
}
