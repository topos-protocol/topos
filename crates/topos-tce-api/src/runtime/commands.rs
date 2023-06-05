use opentelemetry::Context;
use std::collections::HashMap;
use tokio::sync::{mpsc::Sender, oneshot};
use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_core::uci::{Certificate, SubnetId};
use topos_p2p::PeerId;
use uuid::Uuid;

use crate::stream::{Stream, StreamCommand};

use super::error::RuntimeError;

#[derive(Debug)]
pub enum RuntimeCommand {
    /// Dispatch certificate to gRPC API Runtime in order to push it to listening open streams
    DispatchCertificate {
        certificate: Certificate,
        positions: HashMap<SubnetId, TargetStreamPosition>,
    },
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
        subnet_id: SubnetId,
        sender: oneshot::Sender<Result<(u64, Certificate), RuntimeError>>,
    },

    /// Get source head certificate by source subnet id
    GetLastPendingCertificates {
        subnet_ids: Vec<SubnetId>,
        sender: oneshot::Sender<Result<HashMap<SubnetId, Option<Certificate>>, RuntimeError>>,
    },
}
