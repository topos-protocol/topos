use tokio::sync::{mpsc, oneshot};
use topos_p2p::PeerId;

use crate::{GatekeeperCommand, GetAllPeers, GetRandomPeers, PushPeerList};

pub struct GatekeeperClient {
    #[allow(dead_code)]
    pub(crate) shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
    #[allow(dead_code)]
    pub(crate) commands: mpsc::Sender<GatekeeperCommand>,
}

impl GatekeeperClient {
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown_channel.send(sender).await?;

        Ok(receiver.await?)
    }

    pub async fn push_peer_list(
        &self,
        peer_list: Vec<PeerId>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(PushPeerList { peer_list }.send_to(&self.commands).await?)
    }

    pub async fn get_all_peers(&self) -> Result<Vec<PeerId>, Box<dyn std::error::Error>> {
        Ok(GetAllPeers.send_to(&self.commands).await?)
    }

    pub async fn get_random_peers(
        &self,
        number: usize,
    ) -> Result<Vec<PeerId>, Box<dyn std::error::Error>> {
        Ok(GetRandomPeers { number }.send_to(&self.commands).await?)
    }
}
