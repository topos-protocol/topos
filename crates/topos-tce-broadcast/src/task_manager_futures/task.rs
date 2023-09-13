use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use topos_core::uci::CertificateId;
use topos_tce_storage::authority::AuthorityStore;
use topos_tce_storage::errors::StorageError;
use topos_tce_storage::store::{ReadStore, WriteStore};
use topos_tce_storage::types::CertificateDeliveredWithPositions;
use topos_tce_storage::Position;
use tracing::warn;

use crate::double_echo::broadcast_state::{BroadcastState, Status};
use crate::{DoubleEchoCommand, TaskStatus};

#[derive(Debug)]
pub struct TaskContext {
    pub sink: mpsc::Sender<DoubleEchoCommand>,
    pub shutdown_sender: mpsc::Sender<()>,
}

pub struct Task {
    pub authority_store: Arc<AuthorityStore>,
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub certificate_id: CertificateId,
    pub broadcast_state: BroadcastState,
    pub shutdown_receiver: mpsc::Receiver<()>,
    broadcast_sender: broadcast::Sender<CertificateDeliveredWithPositions>,
}

impl Task {
    pub fn new(
        certificate_id: CertificateId,
        broadcast_state: BroadcastState,
        authority_store: Arc<AuthorityStore>,
        broadcast_sender: broadcast::Sender<CertificateDeliveredWithPositions>,
    ) -> (Task, TaskContext) {
        let (message_sender, message_receiver) = mpsc::channel(10_024);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        let task_context = TaskContext {
            sink: message_sender,
            shutdown_sender,
        };

        let task = Task {
            authority_store,
            message_receiver,
            certificate_id,
            broadcast_state,
            shutdown_receiver,
            broadcast_sender,
        };

        (task, task_context)
    }

    pub async fn persist(&self) -> Result<CertificateDeliveredWithPositions, StorageError> {
        let certificate_delivered = self.broadcast_state.into_delivered();

        let positions = self
            .authority_store
            .insert_certificate_delivered(&certificate_delivered)
            .await?;

        Ok(CertificateDeliveredWithPositions(
            certificate_delivered,
            positions,
        ))
    }
}

impl IntoFuture for Task {
    type Output = (CertificateId, TaskStatus);

    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'static>>;

    fn into_future(mut self) -> Self::IntoFuture {
        Box::pin(async move {
            // When the task starts, we need to gather information such as current stream position
            // for the source subnet in order to expect its position
            let expected_position = match self.authority_store.last_delivered_position_for_subnet(
                &self.broadcast_state.certificate.source_subnet_id,
            ) {
                Ok(Some(stream_position)) => stream_position.position,
                Ok(None) => Position(0),
                Err(_) => return (self.certificate_id, TaskStatus::Failure),
            };

            self.broadcast_state.expected_position = Some(expected_position);

            loop {
                tokio::select! {
                    Some(msg) = self.message_receiver.recv() => {
                        match msg {
                            DoubleEchoCommand::Echo { from_peer, .. } => {
                                if let Some(Status::DeliveredWithReadySent) = self.broadcast_state.apply_echo(from_peer) {
                                    match self.persist().await {
                                        Ok(delivered) => {
                                            _ = self.broadcast_sender.send(delivered);

                                            return (self.certificate_id, TaskStatus::Success);
                                        }
                                        Err(error) => {
                                            tracing::error!("Unable to persist one delivered certificate: {:?}", error);
                                            return (self.certificate_id, TaskStatus::Failure);
                                        }
                                    }

                                }
                            }
                            DoubleEchoCommand::Ready { from_peer, .. } => {
                                if let Some(Status::DeliveredWithReadySent) = self.broadcast_state.apply_ready(from_peer) {
                                    match self.persist().await {
                                        Ok(delivered) => {
                                            _ = self.broadcast_sender.send(delivered);

                                            return (self.certificate_id, TaskStatus::Success);
                                        }
                                        Err(error) => {
                                            tracing::error!("Unable to persist one delivered certificate: {:?}", error);
                                            return (self.certificate_id, TaskStatus::Failure);
                                        }
                                    }
                                }
                            }
                            _ => {}
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
