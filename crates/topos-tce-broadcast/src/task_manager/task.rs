use tokio::sync::mpsc;

use topos_core::uci::CertificateId;
use crate::DoubleEchoCommand;

pub(crate) enum TaskStatus {
    /// The task has finished processing the certificate
    Finished,
    /// The task is currently processing the double echo for a certificate
    Processing,
    /// The task has encountered an error while processing the certificate
    Error,
}

/// One unit of work to process the whole lifecycle of a certificate
pub(crate) struct Task {
    /// The id of the task is tightly associated with the certificate it is currently processing
    certificate_id: CertificateId,
    /// The task is receiving DoubleEchoCommands from the parent process which spawned it
    message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    /// The task can send status updates to the parent process which spawned it
    status_sender: mpsc::Sender<TaskStatus>,
    /// The task has to shutdown once the parent process sends a shutdown signal
    shutdown: mpsc::Receiver<()>,
}

