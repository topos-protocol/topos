use std::collections::HashMap;
use std::sync::Arc;

use super::RuntimeCommand;
use futures::Future;
use tokio::sync::{mpsc, oneshot, RwLock};
use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_core::uci::SubnetId;
use topos_core::{api::grpc::tce::v1::StatusResponse, uci::Certificate};
use tracing::error;

#[derive(Clone, Debug)]
pub struct RuntimeClient {
    pub(crate) command_sender: mpsc::Sender<RuntimeCommand>,
    pub(crate) tce_status: Arc<RwLock<StatusResponse>>,
    pub(crate) shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
}

impl RuntimeClient {
    pub fn dispatch_certificate(
        &self,
        certificate: Certificate,
        positions: HashMap<SubnetId, TargetStreamPosition>,
    ) -> impl Future<Output = ()> + 'static + Send {
        let sender = self.command_sender.clone();

        async move {
            if let Err(error) = sender
                .send(RuntimeCommand::DispatchCertificate {
                    certificate,
                    positions,
                })
                .await
            {
                error!("Can't dispatch certificate: {error:?}");
            }
        }
    }

    pub async fn has_active_sample(&self) -> bool {
        self.tce_status.read().await.has_active_sample
    }

    pub async fn set_active_sample(&self, value: bool) {
        let mut status = self.tce_status.write().await;

        status.has_active_sample = value;
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown_channel.send(sender).await?;

        Ok(receiver.await?)
    }
}
