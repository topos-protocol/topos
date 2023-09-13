use std::{collections::HashMap, sync::Arc};

use arc_swap::ArcSwap;
use async_trait::async_trait;
use topos_core::uci::{CertificateId, SubnetId};
use tracing::{error, info};

use crate::{
    authority::AuthorityPerpetualTables,
    epoch::{AuthorityPerEpochStore, EpochParticipantsStore},
    errors::{InternalStorageError, StorageError},
    index::IndexTables,
    rocks::{map::Map, TargetSourceListKey, TargetStreamPositionKey},
    store::{ReadStore, WriteStore},
    types::{CertificateDelivered, SourceStreamPositionKey},
    CertificatePositions, CertificateSourceStreamPosition, CertificateTargetStreamPosition,
    Position, SourceHead,
};

use self::locking::LockGuards;

mod locking;

pub struct FullNodeStore {
    certificate_lock_guards: LockGuards<CertificateId>,
    subnet_lock_guards: LockGuards<SubnetId>,
    #[allow(unused)]
    epoch_store: ArcSwap<AuthorityPerEpochStore>,
    #[allow(unused)]
    participants_store: Arc<EpochParticipantsStore>,
    pub(crate) perpetual_tables: Arc<AuthorityPerpetualTables>,
    pub(crate) index_tables: Arc<IndexTables>,
}

impl FullNodeStore {
    pub fn open(
        epoch_store: ArcSwap<AuthorityPerEpochStore>,
        participants_store: Arc<EpochParticipantsStore>,
        perpetual_tables: Arc<AuthorityPerpetualTables>,
        index_tables: Arc<IndexTables>,
    ) -> Result<Arc<Self>, StorageError> {
        Ok(Arc::new(Self {
            certificate_lock_guards: LockGuards::new(),
            subnet_lock_guards: LockGuards::new(),
            epoch_store,
            participants_store,
            perpetual_tables,
            index_tables,
        }))
    }
}

#[async_trait]
impl WriteStore for FullNodeStore {
    async fn insert_certificate_delivered(
        &self,
        certificate: &CertificateDelivered,
    ) -> Result<CertificatePositions, StorageError> {
        // Lock resources for concurrency issues
        let _cert_guard = self
            .certificate_lock_guards
            .get_lock(certificate.certificate.id)
            .await
            .lock_owned()
            .await;

        let _subnet_guard = self
            .subnet_lock_guards
            .get_lock(certificate.certificate.source_subnet_id)
            .await;

        let subnet_id = certificate.certificate.source_subnet_id;
        let certificate_id = certificate.certificate.id;
        let expected_position = certificate.proof_of_delivery.delivery_position.clone();

        let mut batch = self.perpetual_tables.certificates.batch();
        let mut index_batch = self.index_tables.target_streams.batch();

        // Check position already taken
        if let Some(delivered_at_position) =
            self.perpetual_tables.streams.get(&expected_position)?
        {
            error!(
                "Expected position {} already taken by {}",
                expected_position, delivered_at_position
            );

            return Err(StorageError::InternalStorage(
                InternalStorageError::CertificateAlreadyExistsAtPosition(
                    expected_position.1 .0,
                    expected_position.0,
                ),
            ));
        }

        let update_stream_position = self
            .index_tables
            .source_list
            .get(&subnet_id)?
            .and_then(|(_certificate, pos)| {
                if expected_position.1 .0 > pos.0 {
                    Some((certificate_id, expected_position.1))
                } else {
                    None
                }
            })
            .or(Some((certificate_id, expected_position.1)));

        batch = batch.insert_batch(
            &self.perpetual_tables.certificates,
            [(&certificate_id, certificate)],
        )?;

        // Adding the certificate to the stream
        batch = batch.insert_batch(
            &self.perpetual_tables.streams,
            [(&expected_position, certificate_id)],
        )?;

        index_batch = if let Some(current_source_position) = update_stream_position {
            index_batch.insert_batch(
                &self.index_tables.source_list,
                [(&subnet_id, &current_source_position)],
            )?
        } else {
            index_batch
        };

        // Return list of new target stream positions of certificate that will be persisted
        // Information is needed by sequencer/subnet contract to know from
        // where to continue with streaming on restart
        let mut target_subnet_stream_positions: HashMap<SubnetId, CertificateTargetStreamPosition> =
            HashMap::new();

        // Adding certificate to target_streams
        // TODO: Add expected position instead of calculating on the go
        let mut targets = Vec::new();

        for target_subnet_id in &certificate.certificate.target_subnets {
            let target = if let Some((TargetStreamPositionKey(target, source, position), _)) = self
                .index_tables
                .target_streams
                .prefix_iter(&TargetSourceListKey(*target_subnet_id, subnet_id))?
                .last()
            {
                let target_stream_position = TargetStreamPositionKey(
                    target,
                    source,
                    position.increment().map_err(|error| {
                        InternalStorageError::PositionError(error, subnet_id.into())
                    })?,
                );
                target_subnet_stream_positions.insert(
                    target_stream_position.0,
                    CertificateTargetStreamPosition {
                        target_subnet_id: target_stream_position.0,
                        source_subnet_id: target_stream_position.1,
                        position: target_stream_position.2,
                    },
                );
                (target_stream_position, certificate_id)
            } else {
                let target_stream_position =
                    TargetStreamPositionKey(*target_subnet_id, subnet_id, Position::ZERO);
                target_subnet_stream_positions.insert(
                    target_stream_position.0,
                    CertificateTargetStreamPosition {
                        target_subnet_id: target_stream_position.0,
                        source_subnet_id: target_stream_position.1,
                        position: target_stream_position.2,
                    },
                );

                (target_stream_position, certificate_id)
            };

            let TargetStreamPositionKey(_, _, position) = &target.0;
            index_batch = index_batch.insert_batch(
                &self.index_tables.target_source_list,
                [(TargetSourceListKey(*target_subnet_id, subnet_id), position)],
            )?;

            targets.push(target);
        }

        index_batch = index_batch.insert_batch(&self.index_tables.target_streams, targets)?;

        batch.write()?;
        index_batch.write()?;

        info!(
            "Certificate {} inserted at position {}",
            certificate.certificate.id, expected_position
        );

        Ok(CertificatePositions {
            targets: target_subnet_stream_positions,
            source: expected_position.into(),
        })
    }

    async fn multi_insert_certificates_delivered(
        &self,
        certificates: &[CertificateDelivered],
    ) -> Result<(), StorageError> {
        for certificate in certificates {
            _ = self.insert_certificate_delivered(certificate).await?;
        }
        Ok(())
    }
}

impl ReadStore for FullNodeStore {
    fn get_certificate(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<CertificateDelivered>, StorageError> {
        Ok(self.perpetual_tables.certificates.get(certificate_id)?)
    }

    fn multi_get_certificate(
        &self,
        certificate_ids: &[CertificateId],
    ) -> Result<Vec<Option<CertificateDelivered>>, StorageError> {
        Ok(self
            .perpetual_tables
            .certificates
            .multi_get(certificate_ids)?)
    }

    fn last_delivered_position_for_subnet(
        &self,
        subnet_id: &SubnetId,
    ) -> Result<Option<CertificateSourceStreamPosition>, StorageError> {
        Ok(self
            .perpetual_tables
            .streams
            .prefix_iter(subnet_id)?
            .last()
            .map(|(k, _)| k.into()))
    }

    fn get_checkpoint(&self) -> Result<HashMap<SubnetId, SourceHead>, StorageError> {
        Ok(self
            .index_tables
            .source_list
            .iter()?
            .map(|(subnet_id, (certificate_id, position))| {
                (
                    subnet_id,
                    SourceHead {
                        certificate_id,
                        subnet_id,
                        position,
                    },
                )
            })
            .collect())
    }

    fn get_source_stream_certificates_from_position(
        &self,
        from: SourceStreamPositionKey,
        limit: usize,
    ) -> Result<Vec<(CertificateDelivered, CertificateSourceStreamPosition)>, StorageError> {
        let starting_position = from.1;
        let x: Vec<(CertificateId, CertificateSourceStreamPosition)> = self
            .perpetual_tables
            .streams
            .prefix_iter(&from.0)?
            .skip((starting_position.0).try_into().map_err(|_| {
                StorageError::InternalStorage(InternalStorageError::InvalidQueryArgument(
                    "Unable to parse Position",
                ))
            })?)
            .take(limit)
            .map(|(k, v)| (v, k.into()))
            .collect();

        let certificate_ids: Vec<_> = x.iter().map(|(k, _)| k).cloned().collect();

        let certificates = self
            .perpetual_tables
            .certificates
            .multi_get(&certificate_ids[..])?;

        Ok(x.into_iter()
            .zip(certificates.into_iter())
            .filter_map(|((certificate_id, position), certificate)| {
                certificate
                    .filter(|c| c.certificate.id == certificate_id)
                    .map(|cert| (cert, position))
            })
            .collect())
    }
}
