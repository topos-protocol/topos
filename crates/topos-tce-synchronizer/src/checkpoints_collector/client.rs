use tokio::sync::{mpsc, oneshot};

use super::{CheckpointsCollectorCommand, CheckpointsCollectorError};

pub struct CheckpointsCollectorClient {
    #[allow(dead_code)]
    pub(crate) shutdown_channel: mpsc::Sender<()>,
    #[allow(dead_code)]
    pub(crate) commands: mpsc::Sender<CheckpointsCollectorCommand>,
}

impl CheckpointsCollectorClient {
    pub(crate) async fn start(&self) -> Result<(), CheckpointsCollectorError> {
        let (response_channel, recv) = oneshot::channel();

        self.commands
            .send(CheckpointsCollectorCommand::StartCollecting { response_channel })
            .await?;

        recv.await?
    }
}
