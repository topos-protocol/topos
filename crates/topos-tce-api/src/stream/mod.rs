use futures::{stream::BoxStream, StreamExt, TryStreamExt};
use std::{collections::HashMap, fmt::Debug, time::Duration};
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    time::timeout,
};
use tonic::Status;
use topos_core::uci::{CertificateId, SubnetId};
use topos_tce_types::checkpoints::TargetCheckpoint;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub mod commands;
pub mod errors;

#[cfg(test)]
mod tests;

use crate::{
    grpc::messaging::{
        CertificatePushed, InboundMessage, OpenStream, OutboundMessage, StreamOpened,
    },
    runtime::InternalRuntimeCommand,
    RuntimeError,
};

pub use self::commands::StreamCommand;
pub use self::errors::StreamError;
pub(crate) use self::errors::{HandshakeError, StreamErrorKind};

pub struct Stream {
    pub(crate) stream_id: Uuid,

    pub(crate) target_subnet_listeners:
        HashMap<SubnetId, HashMap<SubnetId, (u64, Option<CertificateId>)>>,

    pub(crate) command_receiver: Receiver<StreamCommand>,
    #[allow(dead_code)]
    pub(crate) internal_runtime_command_sender: Sender<InternalRuntimeCommand>,

    /// gRPC outbound stream
    pub(crate) outbound_stream: Sender<Result<(Option<Uuid>, OutboundMessage), Status>>,
    /// gRPC inbound stream
    pub(crate) inbound_stream:
        BoxStream<'static, Result<(Option<Uuid>, InboundMessage), StreamError>>,
}

impl Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stream")
            .field("stream_id", &self.stream_id)
            .field("target_subnet_listeners", &self.target_subnet_listeners)
            .finish()
    }
}

impl Stream {
    pub(crate) fn new(
        stream_id: Uuid,
        inbound_stream: BoxStream<'static, Result<(Option<Uuid>, InboundMessage), StreamError>>,
        outbound_stream: Sender<Result<(Option<Uuid>, OutboundMessage), Status>>,
        command_receiver: mpsc::Receiver<StreamCommand>,
        internal_runtime_command_sender: Sender<InternalRuntimeCommand>,
    ) -> Self {
        Self {
            stream_id,
            target_subnet_listeners: HashMap::new(),
            command_receiver,
            outbound_stream,
            inbound_stream,
            internal_runtime_command_sender,
        }
    }

    pub async fn run(mut self) -> Result<Uuid, StreamError> {
        // Prestart is the phase that waits for a particular message to being able to process the
        // handshake. For now we do not have authentication nor authorization.
        let (request_id, checkpoint) = self.pre_start().await?;

        // The handshake is preparing the stream to broadcast certificates to the client.
        // Notifying the manager about the subscriptions and defining everything related to
        // the stream management.
        self.handshake(checkpoint)
            .await
            .map_err(|error| StreamError::new(self.stream_id, StreamErrorKind::from(error)))?;

        if let Err(error) = self
            .outbound_stream
            .send(Ok((
                request_id,
                OutboundMessage::StreamOpened(StreamOpened {
                    subnet_ids: self.target_subnet_listeners.keys().copied().collect(),
                }),
            )))
            .await
        {
            error!(%error, "Handshake failed with stream");

            return Err(StreamError::new(
                self.stream_id,
                StreamErrorKind::StreamClosed,
            ));
        }

        loop {
            tokio::select! {
                Some(command) = self.command_receiver.recv() => {
                    if self.handle_command(command).await? {
                        break
                    }
                }

                Some(_stream_packet) = self.inbound_stream.next() => {

                }

                // For graceful shutdown in case streams are closed
                else => break,
            }
        }
        Ok(self.stream_id)
    }
}

impl Stream {
    async fn handle_command(&mut self, command: StreamCommand) -> Result<bool, StreamError> {
        match command {
            StreamCommand::PushCertificate { certificate, .. } => {
                if let Err(error) = self
                    .outbound_stream
                    .send(Ok((
                        None,
                        OutboundMessage::CertificatePushed(Box::new(CertificatePushed {
                            certificate,
                        })),
                    )))
                    .await
                {
                    error!(%error, "Can't forward WatchCertificatesResponse to stream, channel seems dropped");

                    return Err(StreamError::new(
                        self.stream_id,
                        StreamErrorKind::StreamClosed,
                    ));
                }
            }
        }

        Ok(false)
    }

    async fn pre_start(&mut self) -> Result<(Option<Uuid>, TargetCheckpoint), StreamError> {
        let waiting_for_open_stream = async {
            if let Ok(Some((
                request_id,
                InboundMessage::OpenStream(OpenStream {
                    target_checkpoint, ..
                }),
            ))) = self.inbound_stream.try_next().await
            {
                Ok((request_id, target_checkpoint))
            } else {
                Err(())
            }
        };

        match timeout(Duration::from_millis(100), waiting_for_open_stream).await {
            Ok(Ok(checkpoint)) => {
                info!(
                    "Received an OpenStream command for the stream {}",
                    self.stream_id
                );

                Ok(checkpoint)
            }
            Ok(Err(_)) => {
                if let Err(error) = self
                    .outbound_stream
                    .send(Err(Status::invalid_argument("No OpenStream provided")))
                    .await
                {
                    warn!(%error, "Can't notify stream of invalid argument during pre_start");
                    Err(StreamError::new(
                        self.stream_id,
                        StreamErrorKind::StreamClosed,
                    ))
                } else {
                    Err(StreamError::new(
                        self.stream_id,
                        StreamErrorKind::PreStartError,
                    ))
                }
            }
            _ => Err(StreamError::new(self.stream_id, StreamErrorKind::Timeout)),
        }
    }

    async fn handshake(&mut self, checkpoint: TargetCheckpoint) -> Result<(), HandshakeError> {
        _ = self.handle_checkpoint(checkpoint).await;
        let (sender, receiver) = oneshot::channel::<Result<(), RuntimeError>>();

        self.internal_runtime_command_sender
            .send(InternalRuntimeCommand::Register {
                stream_id: self.stream_id,
                subnet_ids: self.target_subnet_listeners.keys().copied().collect(),
                sender,
            })
            .await
            .map_err(Box::new)?;

        receiver.await??;

        self.internal_runtime_command_sender
            .send(InternalRuntimeCommand::Handshaked {
                stream_id: self.stream_id,
            })
            .await
            .map_err(Box::new)?;

        Ok(())
    }

    async fn handle_checkpoint(&mut self, checkpoint: TargetCheckpoint) -> Result<(), StreamError> {
        self.target_subnet_listeners.clear();

        for target in checkpoint.target_subnet_ids {
            self.target_subnet_listeners
                .insert(target, Default::default());
        }

        for position in checkpoint.positions {
            if let Some(entry) = self
                .target_subnet_listeners
                .get_mut(&position.target_subnet_id)
            {
                let optional_cert: Option<CertificateId> = position.certificate_id;
                if entry
                    .insert(
                        position.source_subnet_id,
                        (position.position, optional_cert),
                    )
                    .is_some()
                {
                    debug!(
                        "Stream {} replaced its position for target {:?}",
                        self.stream_id, position.target_subnet_id
                    );
                }
            } else {
                return Err(StreamError::new(
                    self.stream_id,
                    StreamErrorKind::MalformedTargetCheckpoint,
                ));
            }
        }

        info!("{:?}", self.target_subnet_listeners);
        Ok(())
    }
}
