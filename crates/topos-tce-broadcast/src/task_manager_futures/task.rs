use std::future::{Future, IntoFuture};
use std::pin::Pin;
use tokio::sync::mpsc;

use topos_core::uci::CertificateId;
use tracing::warn;

use crate::double_echo::broadcast_state::{BroadcastState, Status};
use crate::{DoubleEchoCommand, TaskStatus};

#[derive(Debug)]
pub struct TaskContext {
    pub sink: mpsc::Sender<DoubleEchoCommand>,
    pub shutdown_sender: mpsc::Sender<()>,
}

#[derive(Debug)]
pub struct Task {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub certificate_id: CertificateId,
    pub broadcast_state: BroadcastState,
    pub shutdown_receiver: mpsc::Receiver<()>,
}

impl Task {
    pub fn new(
        certificate_id: CertificateId,
        broadcast_state: BroadcastState,
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
            broadcast_state,
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
                            DoubleEchoCommand::Echo { from_peer, keypair, signature } => {
                                if let Some(Status::DeliveredWithReadySent) = self.broadcast_state.apply_echo(from_peer, keypair) {
                                    return (self.certificate_id, TaskStatus::Success);
                                }
                            }
                            DoubleEchoCommand::Ready { from_peer, keypair, signature } => {
                                if let Some(Status::DeliveredWithReadySent) = self.broadcast_state.apply_ready(from_peer, keypair) {
                                    return (self.certificate_id, TaskStatus::Success);
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
