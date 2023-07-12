use tokio::sync::mpsc;

use topos_core::uci::CertificateId;

use crate::DoubleEchoCommand;

pub(crate) struct TaskContext {
    pub(crate) certificate_id: CertificateId,
    pub(crate) message_sender: mpsc::Sender<DoubleEchoCommand>,
}

pub(crate) struct Task {
    pub(crate) message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub(crate) certificate_id: CertificateId,
    pub(crate) completion: mpsc::Sender<(bool, CertificateId)>,
}

impl Task {
   pub(crate) fn new(
        certificate_id: CertificateId,
        completion: mpsc::Sender<(bool, CertificateId)>,
    ) -> (Self, TaskContext) {
        let (message_sender, message_receiver) = mpsc::channel(1024);
        let task_context = TaskContext {
            certificate_id,
            message_sender,
        };
        let task = Task {
            message_receiver,
            certificate_id,
            completion,
        };

        (task, task_context)
    }

    async fn handle_msg(&mut self, msg: DoubleEchoCommand) -> Result<bool, ()> {
        match msg {
            DoubleEchoCommand::Echo { .. } => {
                // Do the echo
                // Send the result to the gateway
                return Ok(false);
            }
            DoubleEchoCommand::Ready { certificate_id, .. } => {
                println!("Receive Ready {}", certificate_id);

                self.completion.send((true, self.certificate_id)).await;
                return Ok(true);
                // Do the echo
                // Send the result to the gateway
            }
            _ => todo!()
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                Some(msg) = self.message_receiver.recv() => if let Ok(true) = self.handle_msg(msg).await {
                    break;
                }
            }
        }
    }
}
