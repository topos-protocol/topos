use crate::stream::StreamCommand;
use futures::stream::FuturesUnordered;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;
use topos_api::grpc::checkpoints::TargetStreamPosition;
use topos_core::types::stream::CertificateTargetStreamPosition;
use topos_core::types::CertificateDelivered;
use topos_core::uci::SubnetId;
use topos_tce_storage::{FetchCertificatesFilter, FetchCertificatesPosition, StorageClient};
use tracing::{debug, error, info};
use uuid::Uuid;

type TargetSubnetStreamPositions = HashMap<SubnetId, HashMap<SubnetId, TargetStreamPosition>>;
pub(crate) type RunningTasks =
    FuturesUnordered<Pin<Box<dyn Future<Output = SyncTaskStatus> + Send>>>;

/// Status of a sync task
///
/// When registering a stream, a [`SyncTask`] is started to fetch certificates from the storage
/// and push them to the stream.
#[derive(Debug)]
pub(crate) enum SyncTaskStatus {
    ///  The sync task is active and started running
    Running,
    /// The sync task failed and reported an error
    Error,
    /// The sync task exited gracefully and is done pushing certificates to the stream
    Done,
    /// The sync task was cancelled externally
    Cancelled,
}

/// The [`SyncTask`] is used to fetch certificates from the storage and push them to the stream.
/// It is created when a new stream is registered and is cancelled when a stream with the same Uuid
/// is being started. It is using the [`StorageClient`] to fetch certificates from the storage and
/// a [`Sender`] part of a channel to push certificates to the stream.
pub(crate) struct SyncTask {
    /// The status of the [`SyncTask`]. Can be used to check if the task is still running
    pub(crate) status: SyncTaskStatus,
    /// The stream with which the [`SyncTask`] is connected and pushes certificates to
    pub(crate) stream_id: Uuid,
    /// The positions of each subnet in the stream of where the stream is currently left of
    pub(crate) target_subnet_stream_positions: TargetSubnetStreamPositions,
    /// The connection to the database layer through a StorageClient
    pub(crate) storage: StorageClient,
    /// The notifier is used to send certificates to the stream
    pub(crate) notifier: Sender<StreamCommand>,
    /// If a new stream is registered with the same Uuid, the sync task will be cancelled
    pub(crate) cancel_token: CancellationToken,
}

impl SyncTask {
    /// Creating a new SyncTask which will fetch certificates from the storage and pushes them to the stream
    pub(crate) fn new(
        stream_id: Uuid,
        target_subnet_stream_positions: TargetSubnetStreamPositions,
        storage: StorageClient,
        notifier: Sender<StreamCommand>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            status: SyncTaskStatus::Running,
            stream_id,
            target_subnet_stream_positions,
            storage,
            notifier,
            cancel_token,
        }
    }
}

impl IntoFuture for SyncTask {
    type Output = SyncTaskStatus;

    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'static>>;

    fn into_future(mut self) -> Self::IntoFuture {
        Box::pin(async move {
            info!("Sync task started for stream {}", self.stream_id);
            let mut collector: Vec<(CertificateDelivered, FetchCertificatesPosition)> = Vec::new();

            for (target_subnet_id, source) in &mut self.target_subnet_stream_positions {
                if self.cancel_token.is_cancelled() {
                    self.status = SyncTaskStatus::Cancelled;
                    return SyncTaskStatus::Cancelled;
                }
                let source_subnet_list = self
                    .storage
                    .get_target_source_subnet_list(*target_subnet_id)
                    .await;

                info!(
                    "Stream sync task detected {:?} as source list",
                    source_subnet_list
                );
                if let Ok(source_subnet_list) = source_subnet_list {
                    for source_subnet_id in source_subnet_list {
                        if let Entry::Vacant(entry) = source.entry(source_subnet_id) {
                            entry.insert(TargetStreamPosition {
                                target_subnet_id: *target_subnet_id,
                                source_subnet_id,
                                position: 0,
                                certificate_id: None,
                            });
                        }
                    }
                }

                for TargetStreamPosition {
                    target_subnet_id,
                    source_subnet_id,
                    position,
                    ..
                } in source.values_mut()
                {
                    if self.cancel_token.is_cancelled() {
                        self.status = SyncTaskStatus::Cancelled;
                        return SyncTaskStatus::Cancelled;
                    }
                    if let Ok(certificates_with_positions) = self
                        .storage
                        .fetch_certificates(FetchCertificatesFilter::Target {
                            target_stream_position: CertificateTargetStreamPosition {
                                target_subnet_id: *target_subnet_id,
                                source_subnet_id: *source_subnet_id,
                                position: (*position).into(),
                            },
                            limit: 100,
                        })
                        .await
                    {
                        collector.extend(certificates_with_positions)
                    }
                }
            }

            for (CertificateDelivered { certificate, .. }, position) in collector {
                debug!(
                    "Stream sync task for {} is sending {}",
                    self.stream_id, certificate.id
                );

                if let FetchCertificatesPosition::Target(CertificateTargetStreamPosition {
                    target_subnet_id,
                    source_subnet_id,
                    position,
                }) = position
                {
                    if let Err(e) = self
                        .notifier
                        .send(StreamCommand::PushCertificate {
                            positions: vec![TargetStreamPosition {
                                target_subnet_id,
                                source_subnet_id,
                                position: *position,
                                certificate_id: Some(certificate.id),
                            }],
                            certificate,
                        })
                        .await
                    {
                        error!("Error sending certificate to stream: {}", e);
                        self.status = SyncTaskStatus::Error;
                        return SyncTaskStatus::Error;
                    }
                } else {
                    error!("Invalid certificate position fetched");
                    self.status = SyncTaskStatus::Error;
                    return SyncTaskStatus::Error;
                }
            }

            self.status = SyncTaskStatus::Done;
            SyncTaskStatus::Done
        })
    }
}
