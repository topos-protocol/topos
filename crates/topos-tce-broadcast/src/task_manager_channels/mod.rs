use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};

use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use topos_core::uci::CertificateId;
use tracing::warn;

pub mod task;
use crate::double_echo::broadcast_state::BroadcastState;
use crate::sampler::SubscriptionsView;
use crate::TaskStatus;
use crate::{constant, DoubleEchoCommand};
use task::{Task, TaskContext};

/// The TaskManager is responsible for receiving messages from the network and distributing them
/// among tasks. These tasks are either created if none for a certain CertificateID exists yet,
/// or existing tasks will receive the messages.
pub struct TaskManager {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub task_completion_receiver: mpsc::Receiver<(CertificateId, TaskStatus)>,
    pub task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub notify_task_completion: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub subscription_view_receiver: mpsc::Receiver<SubscriptionsView>,
    pub subscriptions: SubscriptionsView,
    pub event_sender: mpsc::Sender<ProtocolEvents>,
    pub tasks: HashMap<CertificateId, TaskContext>,
    pub buffered_messages: HashMap<CertificateId, Vec<DoubleEchoCommand>>,
    pub thresholds: ReliableBroadcastParams,
    pub shutdown_sender: mpsc::Sender<()>,
}

impl TaskManager {
    pub fn new(
        message_receiver: mpsc::Receiver<DoubleEchoCommand>,
        notify_task_completion: mpsc::Sender<(CertificateId, TaskStatus)>,
        subscription_view_receiver: mpsc::Receiver<SubscriptionsView>,
        event_sender: mpsc::Sender<ProtocolEvents>,
        thresholds: ReliableBroadcastParams,
    ) -> (Self, mpsc::Receiver<()>) {
        let (task_completion_sender, task_completion_receiver) =
            mpsc::channel(*constant::BROADCAST_TASK_COMPLETION_CHANNEL_SIZE);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        (
            Self {
                message_receiver,
                task_completion_receiver,
                task_completion_sender,
                notify_task_completion,
                subscription_view_receiver,
                subscriptions: SubscriptionsView::default(),
                event_sender,
                tasks: HashMap::new(),
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
                    self.subscriptions = new_subscriptions_view;
                }

                Some(msg) = self.message_receiver.recv() => {
                    match msg {
                        DoubleEchoCommand::Echo { certificate_id, .. } | DoubleEchoCommand::Ready{ certificate_id, .. } => {
                            if let Some(task_context) = self.tasks.get(&certificate_id) {
                                _ = task_context.sink.send(msg).await;
                            } else {
                                self.buffered_messages.entry(certificate_id).or_default().push(msg);
                            }
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

                                    let (task, task_context) = Task::new(cert.id, self.task_completion_sender.clone(), broadcast_state);

                                    spawn(task.run());

                                    if let Some(messages) = self.buffered_messages.remove(&cert.id) {
                                        let sink = task_context.sink.clone();
                                        spawn(async move {
                                            for msg in messages {
                                                _ = task_context.sink.send(msg).await;
                                            }
                                        });
                                    }

                                    entry.insert(task_context);
                                }
                                std::collections::hash_map::Entry::Occupied(_) => {},
                            }
                        }
                    }
                }

                Some((certificate_id, status)) = self.task_completion_receiver.recv() => {
                    self.tasks.remove(&certificate_id);
                    let _ = self.notify_task_completion.send((certificate_id, status)).await;
                }

                _ = shutdown_receiver.recv() => {
                    warn!("Task Manager shutting down");

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
