use std::{future::IntoFuture, sync::Arc};

use builder::SynchronizerBuilder;
use checkpoints_collector::{CheckpointsCollectorError, CheckpointsCollectorEvent};
use futures::{future::BoxFuture, FutureExt};
use thiserror::Error;
use tokio::sync::{
    mpsc,
    oneshot::{self, error::RecvError},
};
use tokio_stream::StreamExt;

mod builder;
mod checkpoints_collector;

use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};
use topos_core::{
    api::grpc::{
        shared::v1::positions::SourceStreamPosition,
        tce::v1::{
            synchronizer_service_server::SynchronizerService as GrpcSynchronizerService,
            CheckpointMapFieldEntry, CheckpointRequest, CheckpointResponse,
            FetchCertificatesRequest, FetchCertificatesResponse, ProofOfDelivery, SignedReady,
        },
    },
    uci::CertificateId,
};
use topos_tce_storage::{store::ReadStore, validator::ValidatorStore};
use tracing::{error, info, warn};

pub struct Synchronizer {
    pub(crate) shutdown: CancellationToken,
    #[allow(dead_code)]
    pub(crate) events: mpsc::Sender<SynchronizerEvent>,

    pub(crate) checkpoints_collector_stream: ReceiverStream<CheckpointsCollectorEvent>,
}

impl IntoFuture for Synchronizer {
    type Output = Result<(), SynchronizerError>;

    type IntoFuture = BoxFuture<'static, Self::Output>;

    fn into_future(mut self) -> Self::IntoFuture {
        async move {
            let shutdowned: Option<SynchronizerError> = loop {
                tokio::select! {
                    _ = self.shutdown.cancelled() => {
                        break None
                    }

                    _checkpoint_event = self.checkpoints_collector_stream.next() => {}
                }
            };

            if let Some(_error) = shutdowned {
                warn!("Shutting down Synchronizer due to error...");
            } else {
                info!("Shutting down Synchronizer...");
            }

            Ok(())
        }
        .boxed()
    }
}

impl Synchronizer {
    pub fn builder() -> SynchronizerBuilder {
        SynchronizerBuilder::default()
    }
}

#[derive(Error, Debug)]
pub enum SynchronizerError {
    #[error("Error while dealing with CheckpointsCollector: {0}")]
    CheckpointsCollectorError(#[from] CheckpointsCollectorError),

    #[error("Error while dealing with Start command: unable to start")]
    UnableToStart,

    #[error("Error while dealing with Start command: already starting")]
    AlreadyStarting,

    #[error("Error while dealing with state locking: unable to lock status")]
    UnableToLockStatus,

    #[error(transparent)]
    OneshotCommunicationChannel(#[from] RecvError),

    #[error("Unable to execute shutdown on the Synchronizer: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),

    #[error("No network protocol receiver set")]
    NoProtocolReceiver,
}

pub enum SynchronizerEvent {}

#[derive(Clone)]
pub struct SynchronizerService {
    pub validator_store: Arc<ValidatorStore>,
}

#[async_trait::async_trait]
impl GrpcSynchronizerService for SynchronizerService {
    async fn fetch_certificates(
        &self,
        request: Request<FetchCertificatesRequest>,
    ) -> Result<Response<FetchCertificatesResponse>, Status> {
        let request = request.into_inner();
        let certificate_ids: Vec<CertificateId> = request
            .certificates
            .into_iter()
            .map(|c| c.try_into())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                warn!("Unable to parse certificates: {}", e);
                Status::invalid_argument("Unable to parse certificates")
            })?;

        let response =
            if let Ok(certs) = self.validator_store.get_certificates(&certificate_ids[..]) {
                let certs: Vec<_> = certs
                    .into_iter()
                    .filter_map(|v| v.map(|c| c.certificate.try_into()))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| {
                        warn!("Unable to convert certificates to gRPC type: {}", e);

                        Status::internal("Storage certificates cannot be converted to gRPC type")
                    })?;

                FetchCertificatesResponse {
                    request_id: request.request_id,
                    certificates: certs,
                }
            } else {
                FetchCertificatesResponse {
                    request_id: request.request_id,
                    certificates: vec![],
                }
            };
        Ok(Response::new(response))
    }

    async fn fetch_checkpoint(
        &self,
        request: Request<CheckpointRequest>,
    ) -> Result<Response<CheckpointResponse>, Status> {
        let request = request.into_inner();
        let res: Result<Vec<_>, _> = request
            .checkpoint
            .into_iter()
            .map(|v| v.try_into())
            .collect();

        let res = match res {
            Err(error) => {
                error!("{}", error);
                return Err(Status::invalid_argument("Invalid checkpoint"));
            }
            Ok(value) => value,
        };

        let diff = if let Ok(diff) = self.validator_store.get_checkpoint_diff(res) {
            diff.into_iter()
                .map(|(key, value)| {
                    let v: Vec<_> = value
                        .into_iter()
                        .map(|v| ProofOfDelivery {
                            delivery_position: Some(SourceStreamPosition {
                                source_subnet_id: Some(v.delivery_position.subnet_id.into()),
                                position: *v.delivery_position.position,
                                certificate_id: Some(v.certificate_id.into()),
                            }),
                            readies: v
                                .readies
                                .into_iter()
                                .map(|(ready, signature)| SignedReady { ready, signature })
                                .collect(),
                            threshold: v.threshold,
                        })
                        .collect();
                    CheckpointMapFieldEntry {
                        key: key.to_string(),
                        value: v,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        let response = CheckpointResponse {
            request_id: request.request_id,
            checkpoint_diff: diff,
        };

        Ok(Response::new(response))
    }
}
