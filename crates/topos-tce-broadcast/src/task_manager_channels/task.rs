use tokio::sync::mpsc;

use crate::double_echo::broadcast_state::{BroadcastState, Status};
use crate::DoubleEchoCommand;
use crate::TaskStatus;
use topos_core::uci::CertificateId;

#[derive(Debug, Clone)]
pub struct TaskContext {
    pub sink: mpsc::Sender<DoubleEchoCommand>,
    pub shutdown_sender: mpsc::Sender<()>,
}

pub struct Task {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub certificate_id: CertificateId,
    pub completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub broadcast_state: BroadcastState,
    pub shutdown_receiver: mpsc::Receiver<()>,
}

impl Task {
    pub fn new(
        certificate_id: CertificateId,
        completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
        broadcast_state: BroadcastState,
    ) -> (Self, TaskContext) {
        let (message_sender, message_receiver) = mpsc::channel(1024);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        let task_context = TaskContext {
            sink: message_sender,
            shutdown_sender,
        };

        let task = Task {
            message_receiver,
            certificate_id,
            completion_sender,
            broadcast_state,
            shutdown_receiver,
        };

        (task, task_context)
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                Some(msg) = self.message_receiver.recv() => {
                    match msg {
                        DoubleEchoCommand::Echo { authority_id, .. } => {
                            if let Some(Status::DeliveredWithReadySent) =
                                self.broadcast_state.apply_echo(authority_id)
                            {
                                let _ = self
                                    .completion_sender
                                    .send((self.certificate_id, TaskStatus::Success))
                                    .await;

                                break;
                            }
                        }
                        DoubleEchoCommand::Ready { authority_id, .. } => {
                            if let Some(Status::DeliveredWithReadySent) =
                                self.broadcast_state.apply_ready(authority_id)
                            {
                                let _ = self
                                    .completion_sender
                                    .send((self.certificate_id, TaskStatus::Success))
                                    .await;

                                break;
                            }
                        }
                        _ => {}
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
