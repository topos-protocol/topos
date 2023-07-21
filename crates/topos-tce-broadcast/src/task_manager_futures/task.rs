use std::future::{Future, IntoFuture};
use std::pin::Pin;
use tokio::sync::mpsc;

use topos_core::uci::CertificateId;
use tracing::warn;

use crate::DoubleEchoCommand;
use tce_transport::ReliableBroadcastParams;

#[derive(Debug, PartialEq, Eq)]
pub enum TaskStatus {
    /// The task finished succesfully and broadcasted the certificate + received ready
    Success,
    /// The task did not finish succesfully and stopped.
    Failure,
}

pub struct TaskContext {
    pub sink: mpsc::Sender<DoubleEchoCommand>,
    pub shutdown_sender: mpsc::Sender<()>,
}

pub struct Task {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub certificate_id: CertificateId,
    pub thresholds: ReliableBroadcastParams,
    pub shutdown_receiver: mpsc::Receiver<()>,
}

impl Task {
    pub fn new(
        certificate_id: CertificateId,
        thresholds: ReliableBroadcastParams,
    ) -> (Task, TaskContext) {
        let (message_sender, message_receiver) = mpsc::channel(10_024);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        let task_context = TaskContext {
            sink: message_sender,
            shutdown_sender,
        };

        let task = Task {
            message_receiver,
            certificate_id,
            thresholds,
            shutdown_receiver,
        };

        (task, task_context)
    }
}

impl IntoFuture for Task {
    type Output = (CertificateId, TaskStatus);

    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'static>>;

    fn into_future(mut self) -> Self::IntoFuture {
        Box::pin(async move {
            loop {
                tokio::select! {
                    Some(msg) = self.message_receiver.recv() => {
                        match msg {
                            DoubleEchoCommand::Echo { certificate_id, .. } => {
                                return (certificate_id, TaskStatus::Success);
                            }
                            DoubleEchoCommand::Ready { certificate_id, .. } => {
                                return (certificate_id, TaskStatus::Success);
                            }
                            DoubleEchoCommand::Broadcast { cert, .. } => {
                                return (cert.id, TaskStatus::Success);
                            }

                        }
                    }
                    _ = self.shutdown_receiver.recv() => {
                        warn!("Received shutdown, shutting down task {:?}", self.certificate_id);
                        return (self.certificate_id, TaskStatus::Failure)
                    }
                }
            }
        })
    }
}
