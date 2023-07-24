use futures::stream::FuturesUnordered;
use futures::Future;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::future::IntoFuture;
use std::pin::Pin;
use tokio::sync::mpsc;
use tracing::warn;

use tce_transport::ReliableBroadcastParams;
use topos_core::uci::CertificateId;

pub mod task;

use crate::DoubleEchoCommand;
use task::{Task, TaskContext, TaskStatus};
use topos_p2p::PeerId;

/// The TaskManager is responsible for receiving messages from the network and distributing them
/// among tasks. These tasks are either created if none for a certain CertificateID exists yet,
/// or existing tasks will receive the messages.
pub struct TaskManager {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub tasks: HashMap<CertificateId, TaskContext>,
    #[allow(clippy::type_complexity)]
    pub running_tasks: FuturesUnordered<
        Pin<Box<dyn Future<Output = (CertificateId, TaskStatus)> + Send + 'static>>,
    >,
    pub known_certificates: HashSet<CertificateId>,
    pub buffered_messages: HashMap<CertificateId, Vec<DoubleEchoCommand>>,
    pub thresholds: ReliableBroadcastParams,
    pub shutdown_sender: mpsc::Sender<()>,
}

impl TaskManager {
    pub fn new(
        message_receiver: mpsc::Receiver<DoubleEchoCommand>,
        task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
        thresholds: ReliableBroadcastParams,
    ) -> (Self, mpsc::Receiver<()>) {
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        (
            Self {
                message_receiver,
                task_completion_sender,
                tasks: HashMap::new(),
                running_tasks: FuturesUnordered::new(),
                known_certificates: Default::default(),
                buffered_messages: Default::default(),
                thresholds,
                shutdown_sender,
            },
            shutdown_receiver,
        )
    }

    pub async fn run(mut self, mut shutdown_receiver: mpsc::Receiver<()>) {
        loop {
            tokio::select! {
                Some(msg) = self.message_receiver.recv() => {
                    match msg {
                        DoubleEchoCommand::Echo { certificate_id, from_peer } | DoubleEchoCommand::Ready { certificate_id, from_peer } => {
                            if let task = self.tasks.get(certificate_id) {
                                _ = task.sink.send(msg).await;
                            } else {
                                self.buffered_messages
                                    .entry(certificate_id)
                                    .or_default()
                                    .push(msg);
                            };
                        }
                        DoubleEchoCommand::Broadcast { ref cert, .. } => {
                            let task = match self.tasks.entry(cert.id) {
                                std::collections::hash_map::Entry::Vacant(entry) => {
                                    let (task, task_context) = Task::new(cert.id, self.thresholds.clone());

                                    self.running_tasks.push(task.into_future());

                                    entry.insert(task_context)
                                }
                                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                            };

                            _ = task.sink.send(msg).await;
                        }
                    }
                }
                Some((id, status)) = self.running_tasks.next() => {
                    if status == TaskStatus::Success {
                        self.tasks.remove(&certificate_id);
                        let _ = self.task_completion_sender.send((id, status)).await;
                    }
                }
                _ = shutdown_receiver.recv() => {
                    warn!("Task Manager shutting down");

                    // Shutting down every open task
                    for task in self.tasks.iter() {
                        task.1.shutdown_sender.send(()).await.unwrap();
                    }

                    break;
                }
            }
        }
    }
}

impl Drop for TaskManager {
    fn drop(&mut self) {
        _ = self.shutdown_sender.try_send(());
    }
}
