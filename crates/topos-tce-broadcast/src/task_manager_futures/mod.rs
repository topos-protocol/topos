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

/// The TaskManager is responsible for receiving messages from the network and distributing them
/// among tasks. These tasks are either created if none for a certain CertificateID exists yet,
/// or existing tasks will receive the messages.
pub struct TaskManager {
    pub message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub task_completion_sender: mpsc::Sender<(CertificateId, TaskStatus)>,
    pub tasks: HashMap<CertificateId, TaskContext>,
    pub running_tasks: FuturesUnordered<
        Pin<Box<dyn Future<Output = (CertificateId, TaskStatus)> + Send + 'static>>,
    >,
    pub thresholds: Thresholds,
}

impl TaskManager {
    pub async fn run(mut self) {
        loop {
            // task_completion_sender
            //     .send((certificate_id, TaskStatus::Success))
            //     .await
            //     .unwrap();
            tokio::select! {
                // We receive a new DoubleEchoCommand from the outside through a channel receiver
                // The base state is that there is no running task yet
                // What we need to do is to
                // a) create a new task in the local HashMap: So we can check if incoming messages already have an open task
                // b) Add the task to the FuturesUnordered stream: So we can check if the task is done
                // The task future has to be started for it to be able to listen on the it's own message receiver.
                Some(msg) = self.message_receiver.recv() => {
                    match msg {
                        DoubleEchoCommand::Echo { certificate_id, .. } | DoubleEchoCommand::Ready { certificate_id, ..} => {
                            let task = match self.tasks.entry(certificate_id.clone()) {
                                std::collections::hash_map::Entry::Vacant(entry) => {
                                    let (task, task_context) = Task::new(certificate_id, self.thresholds.clone());

                                    self.running_tasks.push(task.into_future());

                                    entry.insert(task_context)
                                }
                                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                            };

                            _ = task.sink.send(msg).await;
                        }
                        DoubleEchoCommand::Broadcast { ref cert, .. } => {
                            let task = match self.tasks.entry(cert.id.clone()) {
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
                task = self.running_tasks.next() => {
                    if let Some((id, status)) = task {
                        println!("Task {} finished with status {:?}", id, status);
                        let status = self.task_completion_sender.send((id, status)).await;
                        println!("Sent message to channel: {:?}", status);
                    }
                }
            }
        }
    }
}
