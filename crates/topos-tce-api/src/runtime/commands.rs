use tokio::sync::{mpsc::Sender, oneshot};
use tonic::{Status, Streaming};
use topos_core::{
    api::{
        shared::v1::SubnetId,
        tce::v1::{WatchCertificatesRequest, WatchCertificatesResponse},
    },
    uci::Certificate,
};
use uuid::Uuid;

use super::error::RuntimeError;

pub enum RuntimeCommand {
    /// This command is dispatch when a certificate is ready to be dispatch to related subnet
    DispatchCertificate { certificate: Certificate },
}

#[derive(Debug)]
pub(crate) enum InternalRuntimeCommand {
    /// When a new stream is open, this command is dispatch to manage the stream
    NewStream {
        /// Contains the gRPC inbound stream that we need to Poll
        stream: Streaming<WatchCertificatesRequest>,
        /// Sender to the outbound stream to notify and communicate with the client
        sender: Sender<Result<WatchCertificatesResponse, Status>>,
        /// Sender to exchange internal runtime command between streams and runtime
        internal_runtime_command_sender: Sender<Self>,
    },

    /// Register a stream as subscriber for the given subnet_ids.
    /// Commands or certificates pointing to one of the subnet will be forward using the given Sender
    Register {
        stream_id: Uuid,
        subnet_ids: Vec<SubnetId>,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },

    /// Notify that a Stream took too long to handshake
    StreamTimeout { stream_id: Uuid },

    /// Notify that a Stream has successfully handshake with the server
    Handshaked { stream_id: Uuid },

    /// Dispatch when a certificate has been submitted to the TCE.
    /// This command will be used to trigger the DoubleEcho process.
    CertificateSubmitted {
        certificate: Certificate,
        sender: oneshot::Sender<Result<(), RuntimeError>>,
    },
}
