use tokio::sync::mpsc;

use topos_core::uci::CertificateId;

use crate::task_manager::Thresholds;
use crate::DoubleEchoCommand;

pub(crate) struct TaskContext {
    pub(crate) certificate_id: CertificateId,
    pub(crate) message_sender: mpsc::Sender<DoubleEchoCommand>,
}

pub(crate) struct Task {
    pub(crate) message_receiver: mpsc::Receiver<DoubleEchoCommand>,
    pub(crate) certificate_id: CertificateId,
    pub(crate) completion: mpsc::Sender<(bool, CertificateId)>,
    thresholds: Thresholds,
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

        let thresholds = Thresholds {
            echo: 3,
            ready: 3,
            delivery: 3,
        };

        let task = Task {
            message_receiver,
            certificate_id,
            completion,
            thresholds,
        };

        (task, task_context)
    }

    async fn handle_msg(&mut self, msg: DoubleEchoCommand) -> Result<bool, ()> {
        match msg {
            DoubleEchoCommand::Echo { certificate_id, .. } => {
                println!("Receive Echo for: {certificate_id}");

                self.thresholds.echo -= 1;

                if self.thresholds.echo == 0 {
                    self.completion.send((false, self.certificate_id)).await;
                    return Ok(true);
                }
                return Ok(false);
            }
            DoubleEchoCommand::Ready { certificate_id, .. } => {
                println!("Receive Ready {certificate_id}");
                // Do the echo
                // Send the result to the gateway
                self.completion.send((true, self.certificate_id)).await;
                return Ok(true);
            }
            DoubleEchoCommand::Broadcast { cert, .. } => {
                println!("Received certificate via broadcast: {:#?}", cert);
                // Do the broadcast
                // Send the result to the gateway
                return Ok(false);
            }
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
