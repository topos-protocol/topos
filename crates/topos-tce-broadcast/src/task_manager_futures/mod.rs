use futures::stream::FuturesUnordered;
use futures::Future;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::future::IntoFuture;
use std::pin::Pin;
use tokio::sync::mpsc;
use tracing::warn;

use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
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
    pub event_sender: mpsc::Sender<ProtocolEvents>,
    pub tasks: HashMap<CertificateId, TaskContext>,
    #[allow(clippy::type_complexity)]
    pub running_tasks: FuturesUnordered<
        Pin<Box<dyn Future<Output = (CertificateId, TaskStatus)> + Send + 'static>>,
    >,
    pub buffered_messages: HashMap<CertificateId, Vec<DoubleEchoCommand>>,
    pub thresholds: ReliableBroadcastParams,
    pub shutdown_sender: mpsc::Sender<()>,
}

impl TaskManager {
    pub fn new(
        message_receiver: mpsc::Receiver<DoubleEchoCommand>,
        task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
        event_sender: mpsc::Sender<ProtocolEvents>,
        thresholds: ReliableBroadcastParams,
    ) -> (Self, mpsc::Receiver<()>) {
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        (
            Self {
                message_receiver,
                task_completion_sender,
                event_sender,
                tasks: HashMap::new(),
                running_tasks: FuturesUnordered::new(),
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
                            match self.tasks.entry(cert.id) {
                                std::collections::hash_map::Entry::Vacant(entry) => {
                                    let span = warn_span!(
                                        "Broadcast",
                                        peer_id = self.local_peer_id,
                                        certificate_id = cert.id.to_string()
                                    );
                                    DOUBLE_ECHO_BROADCAST_CREATED_TOTAL.inc();
                                    span.in_scope(|| {
                                        warn!("Broadcast registered for {}", cert.id);
                                        self.span_tracker.insert(cert.id, span.clone());
                                        CERTIFICATE_RECEIVED_TOTAL.inc();

                                        if need_gossip {
                                            CERTIFICATE_RECEIVED_FROM_API_TOTAL.inc();
                                        } else {
                                            CERTIFICATE_RECEIVED_FROM_GOSSIP_TOTAL.inc();
                                        }
                                    });

                                    let broadcast_state = BroadcastState::new(
                                        certificate,
                                        self.params.echo_threshold,
                                        self.params.ready_threshold,
                                        self.params.delivery_threshold,
                                        self.event_sender.clone(),
                                        subscriptions,
                                        origin,
                                    );

                                    let (task, task_context) = Task::new(cert.id, self.thresholds.clone(), broadcast_state);

                                    self.running_tasks.push(task.into_future());

                                    entry.insert(task_context)
                                }
                                std::collections::hash_map::Entry::Occupied(entry) => {},
                            }
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

            for (certificate_id, messages) in self.buffered_messages {
                if let Some(task) = self.tasks.get_mut(&certificate_id) {
                    for msg in messages {
                        _ = task.sink.send(msg).await;
                        self.buffered_messages.remove(&certificate_id);
                    }
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
