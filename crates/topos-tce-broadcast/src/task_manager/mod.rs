use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};
use tracing::Span;

use topos_core::uci::CertificateId;

pub(crate) mod task;

use crate::DoubleEchoCommand;
use task::{Task, TaskContext};
use topos_p2p::PeerId;

pub(crate) struct Thresholds {
    pub(crate) echo: usize,
    pub(crate) ready: usize,
    pub(crate) delivery: usize,
}

pub(crate) struct TaskManager {
    pub(crate) message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub(crate) task_completion: mpsc::Receiver<(bool, CertificateId)>,
    pub(crate) task_context: HashMap<CertificateId, TaskContext>,
}

impl TaskManager {
    pub(crate) async fn run(
        mut self,
        task_completion_sender: mpsc::Sender<(bool, CertificateId)>,
        event_sender: mpsc::Sender<task::Events>,
    ) {
        loop {
            tokio::select! {
                Some(task_completion) = self.task_completion.recv() => {
                    println!("Task completed {:?}", task_completion);
                    match task_completion {
                        (true, certificate_id) => {
                            self.task_context.remove(&certificate_id);
                        }
                        (false, certificate_id) => {
                            self.task_context.remove(&certificate_id);
                        }
                    }
                }

                Some(msg) = self.message_receiver.recv() => {
                    // Check if we have task for this certificate_id
                    //      -> if yes forward the message
                    //      -> if no, create a new task and forward the message
                    match msg {
                        DoubleEchoCommand::Echo { certificate_id, .. } | DoubleEchoCommand::Ready{ certificate_id, .. } => {
                            if let Some(task_context) = self.task_context.get(&certificate_id) {

                                let sender = task_context.message_sender.clone();

                                spawn(async move {
                                    _ = sender.send(msg).await;
                                });

                            } else {
                                let (task, context) = Task::new(certificate_id, task_completion_sender.clone(), event_sender.clone());

                                spawn(task.run());

                                let sender = context.message_sender.clone();
                                spawn(async move {
                                    _ = sender.send(msg).await;
                                });
                                self.task_context.insert(certificate_id, context);
                            }
                        }
                        _ => todo!()
                    }
                }
            }
        }
    }
}
