use std::time::Duration;
use tokio::{
    sync::{
        mpsc::{Receiver, Sender},
        oneshot,
    },
    time::timeout,
};
use tokio_stream::StreamExt;
use tonic::{Status, Streaming};
use topos_core::api::{
    shared::v1::SubnetId,
    tce::v1::{
        watch_certificates_request::{Command, OpenStream},
        watch_certificates_response::{CertificatePushed, Event, StreamOpened},
        WatchCertificatesRequest, WatchCertificatesResponse,
    },
};
use tracing::{error, info, warn};
use uuid::Uuid;

pub mod commands;
pub mod errors;
#[cfg(test)]
mod tests;

use crate::runtime::{error::RuntimeError, InternalRuntimeCommand};

use self::errors::HandshakeError;
pub use self::{commands::StreamCommand, errors::PreStartError};

pub struct Stream {
    pub(crate) stream_id: Uuid,
    pub(crate) sender: Sender<Result<WatchCertificatesResponse, Status>>,
    pub(crate) internal_runtime_command_sender: Sender<InternalRuntimeCommand>,
    pub(crate) stream: Streaming<WatchCertificatesRequest>,
    pub(crate) command_receiver: Receiver<StreamCommand>,
}

impl Stream {
    pub async fn run(mut self) {
        let subnet_ids = match self.pre_start().await {
            Err(_) => {
                if let Err(error) = self
                    .sender
                    .send(Err(Status::invalid_argument("No openstream provided")))
                    .await
                {
                    warn!(%error, "Can't notify stream of invalid argument during pre_start");
                }

                if let Err(error) = self
                    .internal_runtime_command_sender
                    .send(InternalRuntimeCommand::StreamTimeout {
                        stream_id: self.stream_id,
                    })
                    .await
                {
                    warn!(%error, "Can't notify stream of timeout during pre_start");
                }

                return;
            }

            Ok(subnet_id) => {
                info!(
                    "Received an OpenStream command for the stream {} linked to the subnet {:?}",
                    self.stream_id, subnet_id
                );

                subnet_id
            }
        };

        if let Err(handshake_error) = self.handshake(subnet_ids.clone()).await {
            error!(%handshake_error, "Handshake failed with stream")
        }

        if let Err(error) = self
            .sender
            .send(Ok(StreamOpened { subnet_ids }.into()))
            .await
        {
            error!(%error, "Handshake failed with stream")
        }

        loop {
            tokio::select! {
                Some(command) = self.command_receiver.recv() => {
                    match command {
                        StreamCommand::PushCertificate { certificate, .. } => {
                            if let Err(error) = self.sender.send(Ok(WatchCertificatesResponse {
                                request_id: None,
                                event: Some(Event::CertificatePushed(CertificatePushed{ certificate: Some(certificate.into()) }))

                            })).await {
                                error!(%error, "Can't forward WatchCertificatesResponse to stream, channel seems dropped");
                            }
                        }
                    }
                }

                Some(_stream_packet) = self.stream.next() => {

                }
            }
        }
    }
}

impl Stream {
    async fn pre_start(&mut self) -> Result<Vec<SubnetId>, PreStartError> {
        let waiting_for_open_stream = async {
            if let Ok(Some(WatchCertificatesRequest {
                command: Some(Command::OpenStream(OpenStream { subnet_ids })),
                ..
            })) = self.stream.message().await
            {
                Ok(subnet_ids)
            } else {
                Err(())
            }
        };

        match timeout(Duration::from_millis(100), waiting_for_open_stream).await {
            Ok(Ok(subnet_id)) => Ok(subnet_id),
            Ok(Err(_)) => Err(PreStartError::WrongOpening),
            _ => Err(PreStartError::TimedOut),
        }
    }

    async fn handshake(&mut self, subnet_ids: Vec<SubnetId>) -> Result<(), HandshakeError> {
        let (sender, receiver) = oneshot::channel::<Result<(), RuntimeError>>();

        self.internal_runtime_command_sender
            .send(InternalRuntimeCommand::Register {
                stream_id: self.stream_id,
                subnet_ids,
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
}
