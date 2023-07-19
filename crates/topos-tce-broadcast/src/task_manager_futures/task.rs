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

#[derive(Debug, PartialEq)]
pub enum TaskStatus {
    /// The task finished succesfully and broadcasted the certificate + received ready
    Success,
    /// The task did not finish succesfully and stopped.
    Failure,
}

pub struct TaskContext {
    pub sink: mpsc::Sender<DoubleEchoCommand>,
    pub message_buffer: Vec<DoubleEchoCommand>,
    pub shutdown_sender: mpsc::Sender<()>,
}

pub struct Task {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub certificate_id: CertificateId,
    pub thresholds: Thresholds,
    pub shutdown_receiver: mpsc::Receiver<()>,
}

impl Task {
    pub fn new(certificate_id: CertificateId, thresholds: Thresholds) -> (Task, TaskContext) {
        let (message_sender, message_receiver) = mpsc::channel(1024);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        let task_context = TaskContext {
            sink: message_sender,
            message_buffer: Vec::new(),
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
                                self.thresholds.echo -= 1;
                                if self.thresholds.echo == 0 {
                                    return (certificate_id.clone(), TaskStatus::Success);
                                }
                            }
                            DoubleEchoCommand::Ready { certificate_id, .. } => {
                                return (certificate_id.clone(), TaskStatus::Success);
                            }
                            DoubleEchoCommand::Broadcast { cert, .. } => {
                                return (cert.id.clone(), TaskStatus::Success);
                            }

                        }
                    }
                    _ = self.shutdown_receiver.recv() => {
                        println!("Received shutdown, shutting down task {:?}", self.certificate_id);
                        return (self.certificate_id, TaskStatus::Failure)
                    }
                }
            }
        })
    }
}
