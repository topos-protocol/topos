use futures::stream::FuturesUnordered;
use futures::Future;
use futures::StreamExt;
use std::collections::HashMap;
use std::future::IntoFuture;
use std::pin::Pin;
use tokio::sync::mpsc;

use topos_core::uci::CertificateId;

pub mod task;

use crate::DoubleEchoCommand;
use task::{Task, TaskContext, TaskStatus};

#[derive(Clone)]
pub struct Thresholds {
    pub echo: usize,
    pub ready: usize,
    pub delivery: usize,
}

pub struct TaskRunner {
    pub running_tasks: FuturesUnordered<
        Pin<Box<dyn Future<Output = (CertificateId, TaskStatus)> + Send + 'static>>,
    >,
    pub task_receiver: mpsc::Receiver<Task>,
    pub task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
}

impl TaskRunner {
    fn new() -> (
        mpsc::Receiver<(CertificateId, TaskStatus)>,
        mpsc::Sender<Task>,
    ) {
        let (task_completion_sender, task_completion_receiver) = mpsc::channel(100);
        let (task_sender, task_receiver) = mpsc::channel(100);

        let runner = Self {
            task_completion_sender,
            task_receiver,
            running_tasks: FuturesUnordered::new(),
        };

        tokio::spawn(async move { runner.run().await });

        (task_completion_receiver, task_sender)
    }
    async fn run(mut self) {
        loop {
            tokio::select! {
                Some(new_task) = self.task_receiver.recv() => {
                    self.running_tasks.push(new_task.into_future());
                }
                Some((id, status)) = self.running_tasks.next() => {
                    let _ = self.task_completion_sender.send((id, status)).await;
                }
            }
        }
    }
}

/// The TaskManager is responsible for receiving messages from the network and distributing them
/// among tasks. These tasks are either created if none for a certain CertificateID exists yet,
/// or existing tasks will receive the messages.
pub struct TaskManager {
    pub task_completed_receiver: mpsc::Receiver<(CertificateId, TaskStatus)>,
    pub tasks: HashMap<CertificateId, TaskContext>,
    pub task_sender: mpsc::Sender<Task>,
    pub thresholds: Thresholds,
}

impl TaskManager {
    pub async fn new(thresholds: Thresholds) -> Self {
        let (task_completed_receiver, task_sender) = TaskRunner::new();
        TaskManager {
            task_completed_receiver,
            task_sender,
            tasks: Default::default(),
            thresholds,
        }
    }
    pub async fn add_message(&mut self, message: DoubleEchoCommand) -> Result<(), ()> {
        match message {
            DoubleEchoCommand::Echo { certificate_id, .. }
            | DoubleEchoCommand::Ready { certificate_id, .. } => {
                let task = match self.tasks.entry(certificate_id.clone()) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let task_context = Task::spawn(
                            self.task_sender.clone(),
                            certificate_id,
                            self.thresholds.clone(),
                        )
                        .await;
                        entry.insert(task_context)
                    }
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                };

                _ = task.sink.send(message).await;
            }
            DoubleEchoCommand::Broadcast { ref cert, .. } => {
                let task = match self.tasks.entry(cert.id.clone()) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let task_context =
                            Task::spawn(self.task_sender.clone(), cert.id, self.thresholds.clone())
                                .await;
                        entry.insert(task_context)
                    }
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                };

                _ = task.sink.send(message).await;
            }
        }

        Ok(())
    }
}
