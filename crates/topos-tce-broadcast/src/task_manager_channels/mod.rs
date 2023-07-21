use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};

use tce_transport::ReliableBroadcastParams;
use topos_core::uci::CertificateId;

pub mod task;

use crate::DoubleEchoCommand;
use task::{Task, TaskContext, TaskStatus};

/// The TaskManager is responsible for receiving messages from the network and distributing them
/// among tasks. These tasks are either created if none for a certain CertificateID exists yet,
/// or existing tasks will receive the messages.
pub struct TaskManager {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub task_completion_receiver: mpsc::Receiver<(CertificateId, TaskStatus)>,
    pub task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub event_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub tasks: HashMap<CertificateId, TaskContext>,
    pub thresholds: ReliableBroadcastParams,
    pub shutdown_sender: mpsc::Sender<()>,
}

impl TaskManager {
    pub fn new(
        message_receiver: mpsc::Receiver<DoubleEchoCommand>,
        event_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
        thresholds: ReliableBroadcastParams,
    ) -> (Self, mpsc::Receiver<()>) {
        let (task_completion_sender, task_completion_receiver) = mpsc::channel(1024);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel(1);

        (
            Self {
                message_receiver,
                task_completion_receiver,
                task_completion_sender,
                event_sender,
                tasks: HashMap::new(),
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
                        DoubleEchoCommand::Echo { certificate_id, .. } | DoubleEchoCommand::Ready{ certificate_id, .. } => {
                            let task_context = match self.tasks.get(&certificate_id) {
                                Some(task_context) => task_context.to_owned(),
                                None => self.create_and_spawn_new_task(certificate_id, self.task_completion_sender.clone()),
                            };

                            _ = task_context.message_sender.send(msg).await;
                        }
                        DoubleEchoCommand::Broadcast { ref cert, .. } => {
                            if self.tasks.get(&cert.id).is_none() {
                                let task_context = self.create_and_spawn_new_task(cert.id, self.task_completion_sender.clone());
                                _ = task_context.message_sender.send(msg).await;
                            }
                        }
                    }
                }

                Some((certificate_id, status)) = self.task_completion_receiver.recv() => {
                    self.tasks.remove(&certificate_id);
                    let _ = self.event_sender.send((certificate_id, status)).await;
                }

                _ = shutdown_receiver.recv() => {
                    println!("Task Manager shutting down");

                    // Shutting down every open task
                    for task in self.tasks.iter() {
                        task.1.shutdown_sender.send(()).await.unwrap();
                    }

                    break;
                }
            }
        }
    }

    fn create_and_spawn_new_task(
        &mut self,
        certificate_id: CertificateId,
        task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    ) -> TaskContext {
        let (task, context) = Task::new(
            certificate_id,
            task_completion_sender,
            self.thresholds.clone(),
        );

        spawn(task.run());

        self.tasks.insert(certificate_id, context.clone());

        context
    }
}

impl Drop for TaskManager {
    fn drop(&mut self) {
        _ = self.shutdown_sender.try_send(());
    }
}
