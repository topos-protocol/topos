use std::error::Error;

use futures::future::BoxFuture;
use libp2p::{request_response::ResponseChannel, PeerId};
use tokio::sync::{mpsc, oneshot};

use crate::{behaviour::transmission::codec::TransmissionResponse, Command};

#[derive(Clone)]
pub struct Client {
    pub local_peer_id: PeerId,
    pub sender: mpsc::Sender<Command>,
}

impl Client {
    pub async fn start_listening(
        &self,
        peer_addr: libp2p::Multiaddr,
    ) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartListening { peer_addr, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }

    pub async fn dial(
        &self,
        peer_id: PeerId,
        peer_addr: libp2p::Multiaddr,
    ) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::Dial {
                peer_id,
                peer_addr,
                sender,
            })
            .await
            .expect("Command receiver not to be dropped.");

        receiver.await.expect("Sender not to be dropped.")
    }

    pub async fn connected_peers(&self) -> Result<Vec<PeerId>, Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::ConnectedPeers { sender })
            .await
            .expect("Command receiver not to be dropped.");

        receiver.await.expect("Sender not to be dropped.")
    }

    pub async fn disconnect(&self) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::Disconnect { sender })
            .await
            .expect("Command receiver not to be dropped.");

        receiver.await.expect("Sender not to be dropped.")
    }

    pub fn send_request<T: Into<Vec<u8>>, R: From<Vec<u8>>>(
        &self,
        to: PeerId,
        data: T,
    ) -> BoxFuture<'static, Result<R, Box<dyn Error + Send>>> {
        let (sender, receiver) = oneshot::channel();

        let network = self.sender.clone();
        let data = data.into();

        Box::pin(async move {
            network
                .send(Command::TransmissionReq { to, data, sender })
                .await
                .expect("Command receiver not to be dropped.");

            receiver
                .await
                .expect("Sender not to be dropped.")
                .map(|result| result.into())
        })
    }

    pub fn respond_to_request<T: Into<Vec<u8>>>(
        &self,
        data: T,
        channel: ResponseChannel<TransmissionResponse>,
    ) -> BoxFuture<'static, ()> {
        let data = data.into();

        let sender = self.sender.clone();

        Box::pin(async move {
            sender
                .send(Command::TransmissionResponse { data, channel })
                .await
                .expect("Command receiver not to be dropped.");
        })
    }
}
