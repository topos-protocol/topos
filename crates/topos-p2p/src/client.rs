use std::sync::Arc;

use futures::future::BoxFuture;
use libp2p::{request_response::ResponseChannel, PeerId};
use tokio::sync::{
    mpsc::{self, error::SendError},
    oneshot,
};

use crate::{
    behaviour::transmission::codec::TransmissionResponse,
    error::{CommandExecutionError, P2PError},
    Command,
};

#[derive(Clone)]
pub struct Client {
    pub local_peer_id: PeerId,
    pub sender: mpsc::Sender<Command>,
    pub kill_switch: Arc<oneshot::Sender<()>>,
}

impl Client {
    pub async fn start_listening(&self, peer_addr: libp2p::Multiaddr) -> Result<(), P2PError> {
        let (sender, receiver) = oneshot::channel();
        let command = Command::StartListening { peer_addr, sender };

        Self::send_command_with_receiver(&self.sender, command, receiver).await
    }

    pub async fn dial(
        &self,
        peer_id: PeerId,
        peer_addr: libp2p::Multiaddr,
    ) -> Result<(), P2PError> {
        let (sender, receiver) = oneshot::channel();
        let command = Command::Dial {
            peer_id,
            peer_addr,
            sender,
        };

        Self::send_command_with_receiver(&self.sender, command, receiver).await
    }

    pub async fn connected_peers(&self) -> Result<Vec<PeerId>, P2PError> {
        let (sender, receiver) = oneshot::channel();
        Self::send_command_with_receiver(&self.sender, Command::ConnectedPeers { sender }, receiver)
            .await
    }

    pub async fn disconnect(&self) -> Result<(), P2PError> {
        let (sender, receiver) = oneshot::channel();
        let command = Command::Disconnect { sender };

        Self::send_command_with_receiver(&self.sender, command, receiver).await
    }

    pub fn send_request<T: Into<Vec<u8>>, R: From<Vec<u8>>>(
        &self,
        to: PeerId,
        data: T,
    ) -> BoxFuture<'static, Result<R, CommandExecutionError>> {
        let data = data.into();
        let network = self.sender.clone();
        let (sender, receiver) = oneshot::channel();
        let (addr_sender, addr_receiver) = oneshot::channel();

        Box::pin(async move {
            Self::send_command_with_receiver(
                &network,
                Command::Discover {
                    to,
                    sender: addr_sender,
                },
                addr_receiver,
            )
            .await?;

            Self::send_command_with_receiver(
                &network,
                Command::TransmissionReq { to, data, sender },
                receiver,
            )
            .await
            .map(|result| result.into())
        })
    }

    pub fn respond_to_request<T: Into<Vec<u8>>>(
        &self,
        data: T,
        channel: ResponseChannel<TransmissionResponse>,
    ) -> BoxFuture<'static, Result<(), CommandExecutionError>> {
        let data = data.into();

        let sender = self.sender.clone();

        Box::pin(async move {
            sender
                .send(Command::TransmissionResponse { data, channel })
                .await
                .map_err(Into::into)
        })
    }

    async fn send_command_with_receiver<
        T,
        E: From<oneshot::error::RecvError> + From<CommandExecutionError>,
    >(
        sender: &mpsc::Sender<Command>,
        command: Command,
        receiver: oneshot::Receiver<Result<T, E>>,
    ) -> Result<T, E> {
        if let Err(SendError(command)) = sender.send(command).await {
            return Err(CommandExecutionError::UnableToSendCommand(command).into());
        }

        receiver.await.unwrap_or_else(|error| Err(error.into()))
    }
}
