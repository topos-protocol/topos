use std::{collections::HashMap, sync::Arc};

use arc_swap::ArcSwap;
use async_trait::async_trait;

use topos_core::{
    types::{
        stream::{CertificateSourceStreamPosition, CertificateTargetStreamPosition, Position},
        CertificateDelivered,
    },
    uci::{CertificateId, SubnetId},
};
use tracing::{error, info};

use crate::{
    epoch::{EpochValidatorsStore, ValidatorPerEpochStore},
    errors::{InternalStorageError, StorageError},
    index::IndexTables,
    rocks::{map::Map, TargetSourceListKey},
    store::{ReadStore, WriteStore},
    validator::ValidatorPerpetualTables,
    CertificatePositions, SourceHead,
};

use self::locking::LockGuards;

pub mod locking;

/// Store to manage FullNode data
///
/// The [`FullNodeStore`] is responsible for storing and exposing the data that is
/// needed by a full node to perform its duties.
///
/// The responsabilities of the [`FullNodeStore`] are:
///
/// - Storing and exposing the certificates that are delivered
/// - Storing and exposing the current state of the streams and the different positions
///
/// To do so, it implements [`ReadStore`] / [`WriteStore`] and use multiple tables and store such
/// as [`ValidatorPerpetualTables`], [`EpochValidatorsStore`] and [`IndexTables`]
pub struct FullNodeStore {
    certificate_lock_guards: LockGuards<CertificateId>,
    subnet_lock_guards: LockGuards<SubnetId>,
    #[allow(unused)]
    epoch_store: ArcSwap<ValidatorPerEpochStore>,
    #[allow(unused)]
    validators_store: Arc<EpochValidatorsStore>,
    pub(crate) perpetual_tables: Arc<ValidatorPerpetualTables>,
    pub(crate) index_tables: Arc<IndexTables>,
}

impl FullNodeStore {
    pub fn open(
        epoch_store: ArcSwap<ValidatorPerEpochStore>,
        validators_store: Arc<EpochValidatorsStore>,
        perpetual_tables: Arc<ValidatorPerpetualTables>,
        index_tables: Arc<IndexTables>,
    ) -> Result<Arc<Self>, StorageError> {
        Ok(Arc::new(Self {
            certificate_lock_guards: LockGuards::new(),
            subnet_lock_guards: LockGuards::new(),
            epoch_store,
            validators_store,
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
                    *expected_position.position,
                    expected_position.subnet_id,
                ),
            ));
        }

        let update_stream_position = self
            .index_tables
            .source_list
            .get(&subnet_id)?
            .and_then(|(_certificate, pos)| {
                if expected_position.position > pos {
                    Some((certificate_id, expected_position.position))
                } else {
                    None
                }
            })
            .or(Some((certificate_id, expected_position.position)));

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
        let source_list_per_target: Vec<_> = certificate
            .certificate
            .target_subnets
            .iter()
            .map(|target_subnet| ((*target_subnet, subnet_id), true))
            .collect();

        for target_subnet_id in &certificate.certificate.target_subnets {
            let target = match self
                .index_tables
                .target_streams
                .prefix_iter(&TargetSourceListKey(*target_subnet_id, subnet_id))?
                .last()
            {
                None => CertificateTargetStreamPosition::new(
                    *target_subnet_id,
                    subnet_id,
                    Position::ZERO,
                ),
                Some((mut target_stream_position, _)) => {
                    target_stream_position.position = target_stream_position
                        .position
                        .increment()
                        .map_err(|error| {
                        InternalStorageError::PositionError(error, subnet_id.into())
                    })?;
                    target_stream_position
                }
            };

            target_subnet_stream_positions.insert(*target_subnet_id, target);

            index_batch = index_batch.insert_batch(
                &self.index_tables.target_source_list,
                [(
                    TargetSourceListKey(*target_subnet_id, subnet_id),
                    target.position,
                )],
            )?;

            targets.push((target, certificate_id));
        }

        index_batch = index_batch.insert_batch(&self.index_tables.target_streams, targets)?;

        index_batch = index_batch.insert_batch(
            &self.index_tables.source_list_per_target,
            source_list_per_target,
        )?;
        batch.write()?;
        index_batch.write()?;

        info!(
            "Certificate {} inserted at position {}",
            certificate.certificate.id, expected_position
        );

        Ok(CertificatePositions {
            targets: target_subnet_stream_positions,
            source: expected_position,
        })
    }

    async fn insert_certificates_delivered(
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
    fn get_source_head(&self, subnet_id: &SubnetId) -> Result<Option<SourceHead>, StorageError> {
        Ok(self
            .index_tables
            .source_list
            .get(subnet_id)?
            .map(|(certificate_id, position)| SourceHead {
                certificate_id,
                subnet_id: *subnet_id,
                position,
            }))
    }

    fn get_certificate(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<CertificateDelivered>, StorageError> {
        Ok(self.perpetual_tables.certificates.get(certificate_id)?)
    }

    fn get_certificates(
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
            .map(|(k, _)| k))
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
        from: CertificateSourceStreamPosition,
        limit: usize,
    ) -> Result<Vec<(CertificateDelivered, CertificateSourceStreamPosition)>, StorageError> {
        let starting_position = from.position;
        let x: Vec<(CertificateId, CertificateSourceStreamPosition)> = self
            .perpetual_tables
            .streams
            .prefix_iter(&from.subnet_id)?
            .skip(starting_position.try_into().map_err(|_| {
                StorageError::InternalStorage(InternalStorageError::InvalidQueryArgument(
                    "Unable to parse Position",
                ))
            })?)
            .take(limit)
            .map(|(k, v)| (v, k))
            .collect();

        let certificate_ids: Vec<_> = x.iter().map(|(k, _)| k).cloned().collect();

        let certificates = self
            .perpetual_tables
            .certificates
            .multi_get(&certificate_ids[..])?;

        Ok(x.into_iter()
            .zip(certificates)
            .filter_map(|((certificate_id, position), certificate)| {
                certificate
                    .filter(|c| c.certificate.id == certificate_id)
                    .map(|cert| (cert, position))
            })
            .collect())
    }

    fn get_target_stream_certificates_from_position(
        &self,
        position: CertificateTargetStreamPosition,
        limit: usize,
    ) -> Result<Vec<(CertificateDelivered, CertificateTargetStreamPosition)>, StorageError> {
        let starting_position = position.position;
        let prefix = TargetSourceListKey(position.target_subnet_id, position.source_subnet_id);

        let certs_with_positions: Vec<(CertificateId, CertificateTargetStreamPosition)> = self
            .index_tables
            .target_streams
            .prefix_iter(&prefix)?
            .skip(starting_position.try_into().map_err(|_| {
                StorageError::InternalStorage(InternalStorageError::InvalidQueryArgument(
                    "Unable to parse Position",
                ))
            })?)
            .take(limit)
            .map(|(k, v)| (v, k))
            .collect();

        let certificate_ids: Vec<_> = certs_with_positions
            .iter()
            .map(|(k, _)| k)
            .cloned()
            .collect();

        let certificates = self
            .perpetual_tables
            .certificates
            .multi_get(&certificate_ids[..])?;

        Ok(certs_with_positions
            .into_iter()
            .zip(certificates)
            .filter_map(|((certificate_id, position), certificate)| {
                certificate
                    .filter(|c| c.certificate.id == certificate_id)
                    .map(|cert| (cert, position))
            })
            .collect())
    }

    fn get_target_source_subnet_list(
        &self,
        target_subnet_id: &SubnetId,
    ) -> Result<Vec<SubnetId>, StorageError> {
        Ok(self
            .index_tables
            .source_list_per_target
            .prefix_iter(target_subnet_id)?
            .map(|((_, source_subnet_id), _)| source_subnet_id)
            .collect())
    }
}
