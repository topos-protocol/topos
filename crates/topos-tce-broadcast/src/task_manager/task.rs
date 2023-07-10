use tokio::sync::mpsc;

use topos_core::uci::CertificateId;
use crate::DoubleEchoCommand;

/// One unit of work to process the whole lifecycle of a certificate
pub(crate) struct Task {
    /// The id of the task is tightly associated with the certificate it is currently processing
    certificate_id: CertificateId,
    /// The task is receiving DoubleEchoCommands from the parent process which spawned it
    message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    /// The task has to shutdown once the parent process sends a shutdown signal
    shutdown: mpsc::Receiver<()>,
}