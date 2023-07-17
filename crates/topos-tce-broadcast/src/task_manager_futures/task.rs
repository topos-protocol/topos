use std::future::{Future, IntoFuture};
use std::pin::Pin;
use tokio::sync::mpsc;

use topos_core::uci::CertificateId;

use crate::task_manager_futures::Thresholds;
use crate::DoubleEchoCommand;

#[derive(Debug, PartialEq)]
pub enum Events {
    ReachedThresholdOfReady(CertificateId),
    ReceivedEcho(CertificateId),
    TimeOut(CertificateId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Success,
    Failure,
}

#[derive(Clone)]
pub struct TaskContext {
    pub sink: mpsc::Sender<DoubleEchoCommand>,
}

pub struct Task {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub certificate_id: CertificateId,
    pub thresholds: Thresholds,
}

impl Task {
    fn new(certificate_id: CertificateId, thresholds: Thresholds) -> (Self, TaskContext) {
        let (message_sender, message_receiver) = mpsc::channel(1024);

        (
            Self {
                certificate_id,
                message_receiver,
                thresholds,
            },
            TaskContext {
                sink: message_sender,
            },
        )
    }

    pub async fn spawn(
        task_sender: mpsc::Sender<Self>,
        certificate_id: CertificateId,
        thresholds: Thresholds,
    ) -> TaskContext {
        let (task, task_context) = Self::new(certificate_id, thresholds);

        if let Err(e) = task_sender.send(task).await {
            panic!("Failed to send task to task runner: {:?}", e);
        }

        task_context
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
                                self.thresholds.echo -= 1;

                                if self.thresholds.echo == 0 {
                                    return (certificate_id.clone(), TaskStatus::Success);
                                }
                            }
                            DoubleEchoCommand::Ready { certificate_id, .. } => {
                                return (certificate_id.clone(), TaskStatus::Success);
                            }
                            DoubleEchoCommand::Broadcast { cert, .. } => {
                                // Do the broadcast
                                // Send the result to the gateway
                                return (cert.id.clone(), TaskStatus::Success);
                            }

                        }
                    }
                }
            }
        })
    }
}
