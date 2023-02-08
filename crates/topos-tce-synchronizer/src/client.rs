use tokio::sync::{mpsc, oneshot};

use crate::{SynchronizerCommand, SynchronizerError};

pub struct SynchronizerClient {
    pub(crate) shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
    #[allow(dead_code)]
    pub(crate) commands: mpsc::Sender<SynchronizerCommand>,
}

impl SynchronizerClient {
    #[allow(dead_code)]
    pub(crate) async fn start(&self) -> Result<(), SynchronizerError> {
        let (response_channel, recv) = oneshot::channel();

        self.commands
            .send(SynchronizerCommand::Start { response_channel })
            .await?;

        recv.await?
    }

    pub async fn shutdown(&self) -> Result<(), SynchronizerError> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown_channel
            .send(sender)
            .await
            .map_err(SynchronizerError::ShutdownCommunication)?;

        Ok(receiver.await?)
    }
}
