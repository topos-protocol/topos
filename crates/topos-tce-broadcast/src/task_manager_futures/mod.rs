use futures::stream::FuturesUnordered;
use futures::Future;
use futures::StreamExt;
use std::collections::HashMap;
use std::future::IntoFuture;
use std::pin::Pin;
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use tokio::sync::{broadcast, mpsc};
use topos_core::uci::CertificateId;
use tracing::warn;

pub mod task;

use crate::double_echo::broadcast_state::BroadcastState;
use crate::sampler::SubscriptionsView;
use crate::DoubleEchoCommand;
use crate::TaskStatus;
use task::{Task, TaskContext};

/// The TaskManager is responsible for receiving messages from the network and distributing them
/// among tasks. These tasks are either created if none for a certain CertificateID exists yet,
/// or existing tasks will receive the messages.
pub struct TaskManager {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub subscription_view_receiver: mpsc::Receiver<SubscriptionsView>,
    pub subscriptions: SubscriptionsView,
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
        subscription_view_receiver: mpsc::Receiver<SubscriptionsView>,
        event_sender: mpsc::Sender<ProtocolEvents>,
        thresholds: ReliableBroadcastParams,
    ) -> (Self, mpsc::Receiver<()>) {
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        (
            Self {
                message_receiver,
                task_completion_sender,
                subscription_view_receiver,
                subscriptions: SubscriptionsView::default(),
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
                biased;

                Some(new_subscriptions_view) = self.subscription_view_receiver.recv() => {
                    println!("Received view");
                    self.subscriptions = new_subscriptions_view;
                }
                Some(msg) = self.message_receiver.recv() => {
                    println!("RECEIVE MESSAGE: {msg:?}");
                    match msg {
                        DoubleEchoCommand::Echo { certificate_id, from_peer } | DoubleEchoCommand::Ready { certificate_id, from_peer } => {
                            if let Some(task) = self.tasks.get(&certificate_id) {
                                _ = task.sink.send(msg).await;
                            } else {
                                self.buffered_messages
                                    .entry(certificate_id)
                                    .or_default()
                                    .push(msg);
                            };
                        }
                        DoubleEchoCommand::Broadcast { ref cert, need_gossip } => {
                            match self.tasks.entry(cert.id) {
                                std::collections::hash_map::Entry::Vacant(entry) => {
                                    let broadcast_state = BroadcastState::new(
                                        cert.clone(),
                                        self.thresholds.echo_threshold,
                                        self.thresholds.ready_threshold,
                                        self.thresholds.delivery_threshold,
                                        self.event_sender.clone(),
                                        self.subscriptions.clone(),
                                        need_gossip,
                                    );

                                    let (task, task_context) = Task::new(cert.id, broadcast_state);

                                    self.running_tasks.push(task.into_future());

                                    entry.insert(task_context);
                                }
                                std::collections::hash_map::Entry::Occupied(entry) => {},
                            }
                        }
                    }
                }


                Some((certificate_id, status)) = self.running_tasks.next() => {
                    if status == TaskStatus::Success {
                        self.tasks.remove(&certificate_id);
                        let _ = self.task_completion_sender.send((certificate_id, status)).await;
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

            for (certificate_id, messages) in &mut self.buffered_messages {
                if let Some(task) = self.tasks.get_mut(&certificate_id) {
                    for msg in messages {
                        _ = task.sink.send(msg.clone()).await;
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
