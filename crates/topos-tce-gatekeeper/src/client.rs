use crate::{GatekeeperCommand, GatekeeperError, GetAllPeers, GetAllSubnets, GetRandomPeers};
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use topos_core::uci::SubnetId;
use topos_p2p::PeerId;

#[async_trait]
pub trait GatekeeperClient: Send + Sync + 'static {
    async fn get_random_peers(&self, number: usize) -> Result<Vec<PeerId>, GatekeeperError>;
}

#[derive(Clone)]
pub struct Client {
    pub(crate) shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
    pub(crate) commands: mpsc::Sender<GatekeeperCommand>,
}

#[async_trait]
impl GatekeeperClient for Client {
    async fn get_random_peers(&self, number: usize) -> Result<Vec<PeerId>, GatekeeperError> {
        GetRandomPeers { number }.send_to(&self.commands).await
    }
}

impl Client {
    pub async fn shutdown(&self) -> Result<(), GatekeeperError> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown_channel
            .send(sender)
            .await
            .map_err(GatekeeperError::ShutdownCommunication)?;

        Ok(receiver.await?)
    }

    pub async fn get_all_peers(&self) -> Result<Vec<PeerId>, GatekeeperError> {
        GetAllPeers.send_to(&self.commands).await
    }

    pub async fn get_all_subnets(&self) -> Result<Vec<SubnetId>, GatekeeperError> {
        GetAllSubnets.send_to(&self.commands).await
    }
}
