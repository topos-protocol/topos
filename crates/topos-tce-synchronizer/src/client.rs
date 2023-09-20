use tokio::sync::{mpsc, oneshot};

use crate::SynchronizerError;

pub struct SynchronizerClient {
    pub(crate) shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
}

impl SynchronizerClient {
    pub async fn shutdown(&self) -> Result<(), SynchronizerError> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown_channel
            .send(sender)
            .await
            .map_err(SynchronizerError::ShutdownCommunication)?;

        Ok(receiver.await?)
    }
}
