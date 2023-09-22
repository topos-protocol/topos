use futures::stream::FuturesUnordered;
use futures::Future;
use futures::StreamExt;
use std::collections::HashMap;
use std::future::IntoFuture;
use std::pin::Pin;
use std::sync::Arc;
use tce_transport::{ProtocolEvents, ReliableBroadcastParams, ValidatorId};
use tokio::sync::broadcast;
use tokio::{spawn, sync::mpsc};
use topos_core::uci::CertificateId;
use topos_metrics::CERTIFICATE_PROCESSING_FROM_API_TOTAL;
use topos_metrics::CERTIFICATE_PROCESSING_FROM_GOSSIP_TOTAL;
use topos_metrics::CERTIFICATE_PROCESSING_TOTAL;
use topos_metrics::DOUBLE_ECHO_ACTIVE_TASKS_COUNT;
use topos_tce_storage::store::ReadStore;
use topos_tce_storage::types::CertificateDeliveredWithPositions;
use topos_tce_storage::validator::ValidatorStore;
use tracing::warn;

pub mod task;

use crate::double_echo::broadcast_state::BroadcastState;
use crate::sampler::SubscriptionsView;
use crate::DoubleEchoCommand;
use crate::TaskStatus;
use task::{Task, TaskContext};
use topos_crypto::messages::MessageSigner;

type RunningTasks =
    FuturesUnordered<Pin<Box<dyn Future<Output = (CertificateId, TaskStatus)> + Send + 'static>>>;

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
    pub message_signer: Arc<MessageSigner>,
    #[allow(clippy::type_complexity)]
    pub running_tasks: RunningTasks,
    pub buffered_messages: HashMap<CertificateId, Vec<DoubleEchoCommand>>,
    pub thresholds: ReliableBroadcastParams,
    pub validator_id: ValidatorId,
    pub shutdown_sender: mpsc::Sender<()>,
    pub validator_store: Arc<ValidatorStore>,
    pub broadcast_sender: broadcast::Sender<CertificateDeliveredWithPositions>,

    pub precedence: HashMap<CertificateId, Task>,
}

impl TaskManager {
    pub fn new(
        message_receiver: mpsc::Receiver<DoubleEchoCommand>,
        task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
        subscription_view_receiver: mpsc::Receiver<SubscriptionsView>,
        event_sender: mpsc::Sender<ProtocolEvents>,
        validator_id: ValidatorId,
        thresholds: ReliableBroadcastParams,
        message_signer: Arc<MessageSigner>,
        validator_store: Arc<ValidatorStore>,
        broadcast_sender: broadcast::Sender<CertificateDeliveredWithPositions>,
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
                validator_id,
                message_signer,
                thresholds,
                shutdown_sender,
                validator_store,
                broadcast_sender,
                precedence: HashMap::new(),
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
                        DoubleEchoCommand::Echo { certificate_id, .. } | DoubleEchoCommand::Ready { certificate_id, .. } => {
                            if let Some(task_context) = self.tasks.get(&certificate_id) {
                                _ = task_context.sink.send(msg).await;
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
                                        self.validator_id,
                                        self.thresholds.echo_threshold,
                                        self.thresholds.ready_threshold,
                                        self.thresholds.delivery_threshold,
                                        self.event_sender.clone(),
                                        self.subscriptions.clone(),
                                        need_gossip,
                                        self.message_signer.clone(),
                                    );

                                    let (task, task_context) = Task::new(
                                        cert.id,
                                        broadcast_state,
                                        self.validator_store.clone(),
                                        self.broadcast_sender.clone()
                                    );

                                    let prev = self.validator_store.get_certificate(&cert.prev_id);
                                    if matches!(prev, Ok(Some(_))) || cert.prev_id == topos_core::uci::INITIAL_CERTIFICATE_ID  {
                                        Self::start_task(
                                            &mut self.running_tasks,
                                            task,
                                            task_context.sink.clone(),
                                            self.buffered_messages.remove(&cert.id),
                                            need_gossip
                                        );
                                    } else {
                                        self.precedence.insert(cert.prev_id, task);
                                    }
                                    entry.insert(task_context);
                                }
                                std::collections::hash_map::Entry::Occupied(_) => {},
                            }
                        }
                    }
                }


                Some((certificate_id, status)) = self.running_tasks.next() => {
                    if let TaskStatus::Success = status {
                        self.tasks.remove(&certificate_id);
                        DOUBLE_ECHO_ACTIVE_TASKS_COUNT.dec();
                        let _ = self.task_completion_sender.send((certificate_id, status)).await;
                        if let Some(task) = self.precedence.remove(&certificate_id) {
                            if let Some(context) = self.tasks.get(&task.certificate_id) {

                                let certificate_id= task.certificate_id;
                                Self::start_task(
                                    &mut self.running_tasks,
                                    task,
                                    context.sink.clone(),
                                    self.buffered_messages.remove(&certificate_id),
                                    false
                                );
                            }


                        }
                    }
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

    fn start_task(
        running_tasks: &mut RunningTasks,
        task: Task,
        sink: mpsc::Sender<DoubleEchoCommand>,
        messages: Option<Vec<DoubleEchoCommand>>,
        need_gossip: bool,
    ) {
        running_tasks.push(task.into_future());

        if let Some(messages) = messages {
            spawn(async move {
                for msg in messages {
                    _ = sink.send(msg).await;
                }
            });
        }

        DOUBLE_ECHO_ACTIVE_TASKS_COUNT.inc();

        CERTIFICATE_PROCESSING_TOTAL.inc();
        if need_gossip {
            CERTIFICATE_PROCESSING_FROM_API_TOTAL.inc();
        } else {
            CERTIFICATE_PROCESSING_FROM_GOSSIP_TOTAL.inc();
        }
    }
}

impl Drop for TaskManager {
    fn drop(&mut self) {
        _ = self.shutdown_sender.try_send(());
    }
}
