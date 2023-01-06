use std::sync::Arc;

use super::RuntimeCommand;
use futures::Future;
use tokio::sync::{mpsc, RwLock};
use topos_core::{api::tce::v1::StatusResponse, uci::Certificate};
use tracing::error;

#[derive(Clone, Debug)]
pub struct RuntimeClient {
    pub(crate) command_sender: mpsc::Sender<RuntimeCommand>,
    pub(crate) tce_status: Arc<RwLock<StatusResponse>>,
}

impl RuntimeClient {
    pub fn dispatch_certificate(
        &self,
        certificate: Certificate,
    ) -> impl Future<Output = ()> + 'static + Send {
        let sender = self.command_sender.clone();

        async move {
            if let Err(error) = sender
                .send(RuntimeCommand::DispatchCertificate { certificate })
                .await
            {
                error!("Can't dispatch certificate: {error:?}");
            }
        }
    }

    pub async fn has_active_sample(&self) {
        let mut status = self.tce_status.write().await;

        status.has_active_sample = true;
    }
}
