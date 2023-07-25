use tokio::sync::mpsc;

use crate::DoubleEchoCommand;
use crate::TaskStatus;
use tce_transport::ReliableBroadcastParams;
use topos_core::uci::CertificateId;

#[derive(Debug, Clone)]
pub struct TaskContext {
    pub message_sender: mpsc::Sender<DoubleEchoCommand>,
    pub shutdown_sender: mpsc::Sender<()>,
}

pub struct Task {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub certificate_id: CertificateId,
    pub completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub thresholds: ReliableBroadcastParams,
    pub shutdown_receiver: mpsc::Receiver<()>,
}

impl Task {
    pub fn new(
        certificate_id: CertificateId,
        completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
        thresholds: ReliableBroadcastParams,
    ) -> (Self, TaskContext) {
        let (message_sender, message_receiver) = mpsc::channel(1024);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        let task_context = TaskContext {
            message_sender,
            shutdown_sender,
        };

        let task = Task {
            message_receiver,
            certificate_id,
            completion_sender,
            thresholds,
            shutdown_receiver,
        };

        (task, task_context)
    }

    async fn handle_msg(&mut self, msg: DoubleEchoCommand) -> (CertificateId, TaskStatus) {
        match msg {
            DoubleEchoCommand::Echo { certificate_id, .. } => {
                let _ = self
                    .completion_sender
                    .send((certificate_id, TaskStatus::Success))
                    .await;

                return (certificate_id, TaskStatus::Success);
            }
            DoubleEchoCommand::Ready { certificate_id, .. } => {
                return (certificate_id, TaskStatus::Success)
            }
            DoubleEchoCommand::Broadcast { cert, .. } => return (cert.id, TaskStatus::Success),
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                msg = self.message_receiver.recv() => {
                    if let Some(msg) = msg {
                        if let (_, TaskStatus::Success) = self.handle_msg(msg).await {
                            break;
                        }
                    }
                }

                _ = self.shutdown_receiver.recv() => {
                    println!("Received shutdown, shutting down task {:?}", self.certificate_id);
                    break;
                }
            }
        }
    }
}
