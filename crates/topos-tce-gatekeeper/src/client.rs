use crate::GatekeeperError;
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct GatekeeperClient {
    pub(crate) shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
}

impl GatekeeperClient {
    pub async fn shutdown(&self) -> Result<(), GatekeeperError> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown_channel
            .send(sender)
            .await
            .map_err(GatekeeperError::ShutdownCommunication)?;

        Ok(receiver.await?)
    }
}
