use crate::stream::StreamCommand;
use futures::stream::FuturesUnordered;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc::Sender;
use topos_api::grpc::checkpoints::TargetStreamPosition;
use topos_core::types::stream::CertificateTargetStreamPosition;
use topos_core::types::CertificateDelivered;
use topos_core::uci::SubnetId;
use topos_tce_storage::{FetchCertificatesFilter, FetchCertificatesPosition, StorageClient};
use tracing::{error, info};
use uuid::Uuid;

type TargetSubnetStreamPositions = HashMap<SubnetId, HashMap<SubnetId, TargetStreamPosition>>;

pub(crate) type SyncTasks =
    FuturesUnordered<Pin<Box<dyn Future<Output = (Uuid, TaskStatus)> + Send + 'static>>>;

/// Status of a sync task
///
/// When registering a stream, a sync task is started to fetch certificates from the storage
/// and push them to the stream.
pub(crate) enum TaskStatus {
    ///  The sync task is active and started running
    Running,
    /// The sync task failed and reported an error
    Error,
    /// The sync task exited gracefully and is done pushing certificates to the stream
    Done,
}

pub(crate) struct Task {
    pub(crate) status: TaskStatus,
    pub(crate) stream_id: Uuid,
    pub(crate) target_subnet_stream_positions: TargetSubnetStreamPositions,
    pub(crate) storage: StorageClient,
    pub(crate) notifier: Sender<StreamCommand>,
}

impl Task {
    pub(crate) fn new(
        stream_id: Uuid,
        target_subnet_stream_positions: TargetSubnetStreamPositions,
        storage: StorageClient,
        notifier: Sender<StreamCommand>,
    ) -> Self {
        Self {
            status: TaskStatus::Running,
            stream_id,
            target_subnet_stream_positions,
            storage,
            notifier,
        }
    }

    pub(crate) async fn run(&mut self) {
        info!("Sync task started for stream {}", self.stream_id);
        let mut collector: Vec<(CertificateDelivered, FetchCertificatesPosition)> = Vec::new();

        for (target_subnet_id, mut source) in self.target_subnet_stream_positions {
            let source_subnet_list = self
                .storage
                .get_target_source_subnet_list(target_subnet_id)
                .await;

            info!(
                "Stream sync task detected {:?} as source list",
                source_subnet_list
            );
            if let Ok(source_subnet_list) = source_subnet_list {
                for source_subnet_id in source_subnet_list {
                    if let Entry::Vacant(entry) = source.entry(source_subnet_id) {
                        entry.insert(TargetStreamPosition {
                            target_subnet_id,
                            source_subnet_id,
                            position: 0,
                            certificate_id: None,
                        });
                    }
                }
            }

            for (
                _,
                TargetStreamPosition {
                    target_subnet_id,
                    source_subnet_id,
                    position,
                    ..
                },
            ) in source
            {
                if let Ok(certificates_with_positions) = self
                    .storage
                    .fetch_certificates(FetchCertificatesFilter::Target {
                        target_stream_position: CertificateTargetStreamPosition {
                            target_subnet_id,
                            source_subnet_id,
                            position: position.into(),
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
            info!(
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
                    self.status = TaskStatus::Error;
                }
            } else {
                error!("Invalid certificate position fetched");
                self.status = TaskStatus::Error;
            }
        }

        self.status = TaskStatus::Done;
    }
}
