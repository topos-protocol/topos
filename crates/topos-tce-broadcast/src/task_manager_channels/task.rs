use std::sync::Arc;
use tokio::sync::mpsc;

use crate::double_echo::broadcast_state::{BroadcastState, Status};
use crate::DoubleEchoCommand;
use crate::TaskStatus;
use topos_core::uci::CertificateId;
use topos_tce_storage::errors::StorageError;
use topos_tce_storage::store::WriteStore;
use topos_tce_storage::validator::ValidatorStore;
use topos_tce_storage::CertificatePositions;

#[derive(Debug, Clone)]
pub struct TaskContext {
    pub sink: mpsc::Sender<DoubleEchoCommand>,
    pub shutdown_sender: mpsc::Sender<()>,
}

pub struct Task {
    pub validator_store: Arc<ValidatorStore>,
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
        validator_store: Arc<ValidatorStore>,
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
            validator_store,
        };

        (task, task_context)
    }

    pub async fn persist(&self) -> Result<CertificatePositions, StorageError> {
        let certificate_delivered = self.broadcast_state.into_delivered();

        self.validator_store
            .insert_certificate_delivered(&certificate_delivered)
            .await
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                Some(msg) = self.message_receiver.recv() => {
                    match msg {
                        DoubleEchoCommand::Echo { from_peer, .. } => {
                            if let Some(Status::DeliveredWithReadySent) =
                                self.broadcast_state.apply_echo(from_peer)
                            {
                                let _ = self
                                    .completion_sender
                                    .send((self.certificate_id, TaskStatus::Success))
                                    .await;

                                break;
                            }
                        }
                        DoubleEchoCommand::Ready { from_peer, .. } => {
                            if let Some(Status::DeliveredWithReadySent) =
                                self.broadcast_state.apply_ready(from_peer)
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
