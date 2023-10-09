use std::time::Duration;

use async_trait::async_trait;
use futures::future::BoxFuture;
use libp2p::{
    request_response::{OutboundFailure, ResponseChannel},
    PeerId,
};
use tokio::sync::{
    mpsc::{self, error::SendError},
    oneshot,
};
use topos_api::grpc::GrpcClient;
use tracing::{debug, info, warn};

use crate::{
    behaviour::transmission::codec::TransmissionResponse,
    error::{CommandExecutionError, P2PError},
    utils::GrpcOverP2P,
    Command,
};

#[async_trait]
pub trait NetworkClient: Send + Sync + 'static {
    async fn new_grpc_client<S: 'static, T: GrpcClient<Output = S> + 'static>(
        &self,
        peer: PeerId,
    ) -> Result<S, P2PError>;

    fn send_request<T: std::fmt::Debug + Into<Vec<u8>> + 'static, R: TryFrom<Vec<u8>> + 'static>(
        &self,
        to: PeerId,
        data: T,
        retry_policy: RetryPolicy,
        protocol: &'static str,
    ) -> BoxFuture<'static, Result<R, CommandExecutionError>>;

    fn respond_to_request<T: std::fmt::Debug + Into<Vec<u8>> + 'static>(
        &self,
        data: Result<T, ()>,
        channel: ResponseChannel<Result<TransmissionResponse, ()>>,
        protocol: &'static str,
    ) -> BoxFuture<'static, Result<(), CommandExecutionError>>;
}

#[derive(Clone)]
pub struct Client {
    pub retry_ttl: u64,
    pub local_peer_id: PeerId,
    pub sender: mpsc::Sender<Command>,
    pub grpc_over_p2p: GrpcOverP2P,
    pub shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
}

impl Client {
    pub async fn start_listening(&self, peer_addr: libp2p::Multiaddr) -> Result<(), P2PError> {
        let (sender, receiver) = oneshot::channel();
        let command = Command::StartListening { peer_addr, sender };

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

    pub fn publish<T: std::fmt::Debug + Into<Vec<u8>>>(
        &self,
        topic: &'static str,
        data: T,
    ) -> BoxFuture<'static, Result<(), SendError<Command>>> {
        let data = data.into();
        let network = self.sender.clone();

        Box::pin(async move { network.send(Command::Gossip { topic, data }).await })
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

    pub async fn shutdown(&self) -> Result<(), P2PError> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown_channel
            .send(sender)
            .await
            .map_err(P2PError::ShutdownCommunication)?;

        Ok(receiver.await?)
    }
}

#[async_trait]
impl NetworkClient for Client {
    async fn new_grpc_client<S: 'static, T: GrpcClient<Output = S>>(
        &self,
        peer: PeerId,
    ) -> Result<S, P2PError> {
        Ok(self
            .grpc_over_p2p
            .create::<T>(peer)
            .await
            .map_err(|_| P2PError::UnableToCreateGrpcClient)?)
    }

    fn respond_to_request<T: std::fmt::Debug + Into<Vec<u8>>>(
        &self,
        data: Result<T, ()>,
        channel: ResponseChannel<Result<TransmissionResponse, ()>>,
        protocol: &'static str,
    ) -> BoxFuture<'static, Result<(), CommandExecutionError>> {
        let data = data.map(Into::into);

        let sender = self.sender.clone();

        Box::pin(async move {
            sender
                .send(Command::TransmissionResponse {
                    data,
                    channel,
                    protocol,
                })
                .await
                .map_err(Into::into)
        })
    }

    fn send_request<T: std::fmt::Debug + Into<Vec<u8>>, R: TryFrom<Vec<u8>>>(
        &self,
        to: PeerId,
        data: T,
        retry_policy: RetryPolicy,
        protocol: &'static str,
    ) -> BoxFuture<'static, Result<R, CommandExecutionError>> {
        let data = data.into();
        let network = self.sender.clone();

        let ttl = self.retry_ttl;
        Box::pin(async move {
            let mut retry_count = match retry_policy {
                RetryPolicy::NoRetry => 0,
                RetryPolicy::N(n) => n,
            };

            loop {
                let (addr_sender, addr_receiver) = oneshot::channel();
                match Self::send_command_with_receiver(
                    &network,
                    Command::Discover {
                        to,
                        sender: addr_sender,
                    },
                    addr_receiver,
                )
                .await
                {
                    Err(e) if retry_count == 0 => {
                        warn!(
                            "Fail to send discovery query to {} because of error {e:?}",
                            to
                        );
                        return Err(e);
                    }
                    Err(e) => {
                        retry_count -= 1;
                        debug!("Retry query because of failure {e:?} during discovery phase");
                        tokio::time::sleep(Duration::from_millis(ttl)).await;
                    }
                    Ok(_) => break,
                }
            }
            let mut retry_count = match retry_policy {
                RetryPolicy::NoRetry => 0,
                RetryPolicy::N(n) => n,
            };
            loop {
                let (sender, receiver) = oneshot::channel();
                match Self::send_command_with_receiver(
                    &network,
                    Command::TransmissionReq {
                        to,
                        data: data.clone(),
                        protocol,
                        sender,
                    },
                    receiver,
                )
                .await
                {
                    Err(e) if retry_count == 0 => {
                        warn!("Fail to send query to {} because of error {e:?}", to);
                        return Err(e);
                    }
                    Err(e) => {
                        retry_count -= 1;
                        // Note: Currently UnsupportedProtocols is returned when the peer is not able to handle the request
                        if !matches!(
                            e,
                            CommandExecutionError::RequestOutbandFailure(
                                OutboundFailure::UnsupportedProtocols
                            )
                        ) {
                            info!("Retry query because of failure {e:?}");
                        }
                        tokio::time::sleep(Duration::from_millis(ttl)).await;
                    }
                    Ok(res) => {
                        return res
                            .try_into()
                            .map_err(|_| CommandExecutionError::ParsingError)
                    }
                }
            }
        })
    }
}

pub enum RetryPolicy {
    NoRetry,
    N(usize),
}
