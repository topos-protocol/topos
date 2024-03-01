//! Validator's context store and storage
//!
//! The [`ValidatorStore`] is responsible for managing the various kind of data that are required by the
//! TCE network in order to broadcast certificates. It is composed of two main parts:
//!
//! - a [`FullNodeStore`]
//! - a [`ValidatorPendingTables`]
//!
//! ## Responsibilities
//!
//! This store is used in places where the [`FullNodeStore`] is not enough, it allows to access the
//! different pending pools and to manage them but also to access the [`FullNodeStore`] in order to
//! persist or update [`Certificate`] or `streams`.
//!
//! Pending pools and their behavior are described in the [`ValidatorPendingTables`] documentation.
//!
use std::{
    collections::HashMap,
    path::Path,
    sync::{atomic::Ordering, Arc},
};

use async_trait::async_trait;

use rocksdb::properties::ESTIMATE_NUM_KEYS;
use topos_core::{
    types::{
        stream::{CertificateSourceStreamPosition, Position},
        CertificateDelivered, ProofOfDelivery,
    },
    uci::{Certificate, CertificateId, SubnetId, INITIAL_CERTIFICATE_ID},
};
use topos_metrics::{STORAGE_PENDING_POOL_COUNT, STORAGE_PRECEDENCE_POOL_COUNT};
use tracing::{debug, error, info, instrument, warn};

use crate::{
    errors::{InternalStorageError, StorageError},
    fullnode::FullNodeStore,
    rocks::map::Map,
    store::{ReadStore, WriteStore},
    CertificatePositions, CertificateTargetStreamPosition, PendingCertificateId, SourceHead,
};

pub use self::tables::ValidatorPendingTables;
pub use self::tables::ValidatorPerpetualTables;

mod tables;

/// Store to manage Validator data
///
/// The [`ValidatorStore`] is composed of a [`FullNodeStore`] and a [`ValidatorPendingTables`].
///
/// As the [`FullNodeStore`] is responsible of keeping and managing data that are persistent,
/// the [`ValidatorStore`] is delegating to it many of the [`WriteStore`] and [`ReadStore`]
/// functionality.
///
/// The key point is that the [`ValidatorStore`] is managing the different pending pools using a [`ValidatorPendingTables`].
///
/// Pending pools and how they behave are described in the [`ValidatorPendingTables`] documentation.
///
pub struct ValidatorStore {
    pub(crate) pending_tables: ValidatorPendingTables,
    pub(crate) fullnode_store: Arc<FullNodeStore>,
}

impl ValidatorStore {
    /// Try to create a new instance of [`ValidatorStore`] based on the given path
    pub fn new(path: &Path) -> Result<Arc<Self>, StorageError> {
        let fullnode_store = FullNodeStore::new(path)?;

        Self::open(path, fullnode_store)
    }

    /// Open a [`ValidatorStore`] at the given `path` and using the given [`FullNodeStore`]
    pub fn open(
        path: &Path,
        fullnode_store: Arc<FullNodeStore>,
    ) -> Result<Arc<Self>, StorageError> {
        let pending_tables: ValidatorPendingTables = ValidatorPendingTables::open(path);

        let store = Arc::new(Self {
            pending_tables,
            fullnode_store,
        });

        store.pending_tables.pending_pool.rocksdb.compact_range_cf(
            &store.pending_tables.pending_pool.cf()?,
            None::<&[u8]>,
            None::<&[u8]>,
        );
        store
            .pending_tables
            .precedence_pool
            .rocksdb
            .compact_range_cf(
                &store.pending_tables.precedence_pool.cf()?,
                None::<&[u8]>,
                None::<&[u8]>,
            );

        let pending_count: i64 = store.pending_pool_size()?.try_into().map_err(|error| {
            error!("Failed to convert estimate-num-keys to i64: {}", error);
            StorageError::InternalStorage(InternalStorageError::UnexpectedDBState(
                "Failed to convert estimate-num-keys to i64",
            ))
        })?;

        let precedence_count: i64 = store.precedence_pool_size()?.try_into().map_err(|error| {
            error!("Failed to convert estimate-num-keys to i64: {}", error);
            StorageError::InternalStorage(InternalStorageError::UnexpectedDBState(
                "Failed to convert estimate-num-keys to i64",
            ))
        })?;

        STORAGE_PENDING_POOL_COUNT.set(pending_count);
        STORAGE_PRECEDENCE_POOL_COUNT.set(precedence_count);

        Ok(store)
    }

    /// Returns the [`FullNodeStore`] used by the [`ValidatorStore`]
    pub fn get_fullnode_store(&self) -> Arc<FullNodeStore> {
        self.fullnode_store.clone()
    }

    /// Returns the number of certificates in the pending pool
    pub fn pending_pool_size(&self) -> Result<u64, StorageError> {
        Ok(self
            .pending_tables
            .pending_pool
            .property_int_value(ESTIMATE_NUM_KEYS)?)
    }

    /// Returns the number of certificates in the precedence pool
    pub fn precedence_pool_size(&self) -> Result<u64, StorageError> {
        Ok(self
            .pending_tables
            .precedence_pool
            .property_int_value(ESTIMATE_NUM_KEYS)?)
    }

    /// Try to return the [`PendingCertificateId`] for a [`CertificateId`]
    ///
    /// Return `Ok(None)` if the `certificate_id` is not found.
    pub fn get_pending_id(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<PendingCertificateId>, StorageError> {
        Ok(self.pending_tables.pending_pool_index.get(certificate_id)?)
    }

    /// Try to return the [`Certificate`] for a [`PendingCertificateId`]
    ///
    /// Return `Ok(None)` if the `pending_id` is not found.
    pub fn get_pending_certificate(
        &self,
        pending_id: &PendingCertificateId,
    ) -> Result<Option<Certificate>, StorageError> {
        Ok(self.pending_tables.pending_pool.get(pending_id)?)
    }

    /// Returns an iterator over the pending pool
    ///
    /// Note: this can be slow on large datasets.
    #[doc(hidden)]
    pub fn iter_pending_pool(
        &self,
    ) -> Result<impl Iterator<Item = (PendingCertificateId, Certificate)> + '_, StorageError> {
        Ok(self.pending_tables.pending_pool.iter()?)
    }

    /// Returns an iterator over the pending pool starting at a given `PendingCertificateId`
    ///
    /// Note: this can be slow on large datasets.
    #[doc(hidden)]
    pub fn iter_pending_pool_at(
        &self,
        pending_id: &PendingCertificateId,
    ) -> Result<impl Iterator<Item = (PendingCertificateId, Certificate)> + '_, StorageError> {
        Ok(self.pending_tables.pending_pool.iter_at(pending_id)?)
    }

    /// Returns an iterator over the precedence pool
    ///
    /// Note: this can be slow on large datasets.
    #[doc(hidden)]
    pub fn iter_precedence_pool(
        &self,
    ) -> Result<impl Iterator<Item = (CertificateId, Certificate)> + '_, StorageError> {
        Ok(self.pending_tables.precedence_pool.iter()?)
    }

    pub fn get_next_pending_certificates(
        &self,
        from: &PendingCertificateId,
        number: usize,
    ) -> Result<Vec<(PendingCertificateId, Certificate)>, StorageError> {
        debug!(
            "Get next pending certificates from {} (max: {})",
            from, number
        );
        Ok(self
            .pending_tables
            .pending_pool
            .iter_at(from)?
            .take(number)
            .collect())
    }

    /// Returns the [Certificate] (if any) that is currently in the precedence pool for the given [CertificateId]
    pub fn check_precedence(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<Certificate>, StorageError> {
        Ok(self.pending_tables.precedence_pool.get(certificate_id)?)
    }

    // TODO: Performance issue on this one as we iter over all the pending certificates
    // We need to improve how we request the pending certificates.
    pub fn get_pending_certificates_for_subnets(
        &self,
        subnets: &[SubnetId],
    ) -> Result<HashMap<SubnetId, (u64, Option<Certificate>)>, StorageError> {
        let mut result: HashMap<SubnetId, (u64, Option<Certificate>)> = subnets
            .iter()
            .enumerate()
            .map(|(_, s)| (*s, (0, None)))
            .collect();

        for (_, certificate) in self.pending_tables.pending_pool.iter()? {
            if !subnets.contains(&certificate.source_subnet_id) {
                continue;
            }

            let mut latest_cert = certificate;
            let entry = result
                .entry(latest_cert.source_subnet_id)
                .or_insert((0, None));

            entry.0 += 1;
            while let Some(certificate) =
                self.pending_tables.precedence_pool.get(&latest_cert.id)?
            {
                latest_cert = certificate;
                entry.0 += 1;
            }

            entry.1 = Some(latest_cert);
        }

        Ok(result)
    }

    #[cfg(test)]
    pub(crate) fn insert_pending_certificates(
        &self,
        certificates: &[Certificate],
    ) -> Result<Vec<PendingCertificateId>, StorageError> {
        let id = self
            .pending_tables
            .next_pending_id
            .fetch_add(certificates.len() as u64, Ordering::Relaxed);

        let mut batch = self.pending_tables.pending_pool.batch();

        let (values, index, ids) = certificates.iter().enumerate().fold(
            (Vec::new(), Vec::new(), Vec::new()),
            |(mut values, mut index, mut ids), (idx, certificate)| {
                let id = id + idx as u64;

                index.push((certificate.id, id));
                values.push((id, certificate));
                ids.push(id);

                (values, index, ids)
            },
        );

        batch = batch.insert_batch(&self.pending_tables.pending_pool, values)?;
        batch = batch.insert_batch(&self.pending_tables.pending_pool_index, index)?;

        batch.write()?;

        STORAGE_PENDING_POOL_COUNT.add(ids.len() as i64);

        Ok(ids)
    }

    pub async fn insert_pending_certificate(
        &self,
        certificate: &Certificate,
    ) -> Result<Option<PendingCertificateId>, StorageError> {
        // A lock guard is asked during the insertion of a pending certificate
        // to avoid race condition when a certificate is being inserted and added
        // to the pending pool at the same time
        let _certificate_guard = self
            .fullnode_store
            .get_certificate_lock_guard(certificate.id)
            .await;

        if self.get_certificate(&certificate.id)?.is_some() {
            debug!("Certificate {} is already delivered", certificate.id);
            return Err(StorageError::InternalStorage(
                InternalStorageError::CertificateAlreadyExists,
            ));
        }

        if self
            .pending_tables
            .pending_pool_index
            .get(&certificate.id)?
            .is_some()
        {
            debug!(
                "Certificate {} is already in the pending pool",
                certificate.id
            );
            return Err(StorageError::InternalStorage(
                InternalStorageError::CertificateAlreadyPending,
            ));
        }

        // A lock guard is asked during the insertion of a pending certificate
        // to avoid race condition when a certificate is being added to the
        // pending pool while its parent is currently being inserted as delivered
        let _prev_certificate_guard = self
            .fullnode_store
            .get_certificate_lock_guard(certificate.prev_id)
            .await;

        let prev_delivered = certificate.prev_id == INITIAL_CERTIFICATE_ID
            || self.get_certificate(&certificate.prev_id)?.is_some();

        if prev_delivered {
            let id = self
                .pending_tables
                .next_pending_id
                .fetch_add(1, Ordering::Relaxed);

            self.pending_tables.pending_pool.insert(&id, certificate)?;
            self.pending_tables
                .pending_pool_index
                .insert(&certificate.id, &id)?;

            STORAGE_PENDING_POOL_COUNT.inc();
            debug!(
                "Certificate {} is now in the pending pool at index: {}",
                certificate.id, id
            );
            Ok(Some(id))
        } else {
            self.pending_tables
                .precedence_pool
                .insert(&certificate.prev_id, certificate)?;

            STORAGE_PRECEDENCE_POOL_COUNT.inc();
            debug!(
                "Certificate {} is now in the precedence pool, because the previous certificate \
                 {} isn't delivered yet",
                certificate.id, certificate.prev_id
            );

            Ok(None)
        }
    }

    #[instrument(skip(self, proofs))]
    pub fn insert_unverified_proofs(
        &self,
        proofs: Vec<ProofOfDelivery>,
    ) -> Result<Vec<CertificateId>, StorageError> {
        let certs: Vec<CertificateId> = proofs.iter().map(|proof| proof.certificate_id).collect();

        let unverified: Vec<(CertificateId, ProofOfDelivery)> = proofs
            .into_iter()
            .map(|proof| {
                debug!(
                    "Certificate Sync: unverified proof for {} inserted",
                    proof.certificate_id
                );
                (proof.certificate_id, proof)
            })
            .collect();

        self.fullnode_store
            .perpetual_tables
            .unverified
            .multi_insert(unverified)?;

        Ok(certs)
    }

    #[instrument(skip(self, certificate))]
    pub async fn synchronize_certificate(
        &self,
        certificate: Certificate,
    ) -> Result<(), StorageError> {
        if let Ok(Some(proof_of_delivery)) = self.get_unverified_proof(&certificate.id) {
            let certificate_id = certificate.id;
            debug!(
                "Certificate Sync: certificate {} is now defined as delivered",
                certificate_id
            );
            self.insert_certificate_delivered(&CertificateDelivered {
                certificate,
                proof_of_delivery,
            })
            .await?;

            debug!(
                "Certificate Sync: unverified proof has been removed for {}",
                certificate_id
            );
            self.fullnode_store
                .perpetual_tables
                .unverified
                .delete(&certificate_id)?;

            Ok(())
        } else {
            debug!("Certificate Sync: Proof not found for {}", certificate.id);
            Err(StorageError::InternalStorage(
                crate::errors::InternalStorageError::InvalidQueryArgument("Proof not found"),
            ))
        }
    }

    pub fn get_unverified_proof(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<ProofOfDelivery>, StorageError> {
        Ok(self
            .fullnode_store
            .perpetual_tables
            .unverified
            .get(certificate_id)?)
    }

    /// Returns the difference between the `from` list of [ProofOfDelivery] and the local head
    /// checkpoint. This is used to define the list of certificates that are missing between the
    /// `from` and the local head checkpoint.
    /// The maximum number of [ProofOfDelivery] returned per [SubnetId] is 100.
    /// If the `from` is missing a local subnet, the list of [ProofOfDelivery] for this subnet will
    /// start from [Position] `0`.
    pub fn get_checkpoint_diff(
        &self,
        from: &[ProofOfDelivery],
        limit_per_subnet: usize,
    ) -> Result<HashMap<SubnetId, Vec<ProofOfDelivery>>, StorageError> {
        // Parse the from in order to extract the different position per subnets
        let from_positions: HashMap<SubnetId, &ProofOfDelivery> = from
            .iter()
            .map(|v| (v.delivery_position.subnet_id, v))
            .collect();

        let mut output: HashMap<SubnetId, Vec<ProofOfDelivery>> = HashMap::new();

        // Request the local head checkpoint
        let subnets: HashMap<SubnetId, Position> = self
            .fullnode_store
            .index_tables
            .source_list
            .iter()?
            .map(|(subnet_id, (_, position))| (subnet_id, position))
            .collect();

        // For every local known subnets we want to iterate and check if there
        // is a delta between the from_position and our head position.
        for (subnet, local_position) in subnets {
            let certs: Vec<_> = if let Some(position) = from_positions.get(&subnet) {
                if local_position <= position.delivery_position.position {
                    continue;
                }

                self.fullnode_store
                    .perpetual_tables
                    .streams
                    .prefix_iter(&(&subnet, &position.delivery_position.position))?
                    .skip(1)
                    .take(limit_per_subnet)
                    .map(|(_, v)| v)
                    .collect()
            } else {
                self.fullnode_store
                    .perpetual_tables
                    .streams
                    .prefix_iter(&(&subnet, Position::ZERO))?
                    .take(limit_per_subnet)
                    .map(|(_, v)| v)
                    .collect()
            };

            let proofs: Vec<_> = self
                .fullnode_store
                .get_certificates(&certs)?
                .into_iter()
                .filter_map(|v| v.map(|c| c.proof_of_delivery))
                .collect();

            info!(
                "Certificate Sync: distance between from and head for {} subnet is {}",
                subnet,
                proofs.len()
            );

            if let Some(old_value) = output.insert(subnet, proofs) {
                error!(
                    "Certificate Sync: This should not happen, we are overwriting a value during \
                     sync of {subnet}. Overwriting {}",
                    old_value.len()
                );
            }
        }

        Ok(output)
    }

    #[cfg(test)]
    pub(crate) fn delete_pending_certificate(
        &self,
        pending_id: &PendingCertificateId,
    ) -> Result<Certificate, StorageError> {
        if let Some(certificate) = self.pending_tables.pending_pool.get(pending_id)? {
            self.pending_tables.pending_pool.delete(pending_id)?;
            self.pending_tables
                .pending_pool_index
                .delete(&certificate.id)?;

            STORAGE_PENDING_POOL_COUNT.dec();
            Ok(certificate)
        } else {
            Err(StorageError::InternalStorage(
                crate::errors::InternalStorageError::InvalidQueryArgument(
                    "No certificate for pending_id",
                ),
            ))
        }
    }
}
impl ReadStore for ValidatorStore {
    fn count_certificates_delivered(&self) -> Result<u64, StorageError> {
        self.fullnode_store.count_certificates_delivered()
    }

    fn get_source_head(&self, subnet_id: &SubnetId) -> Result<Option<SourceHead>, StorageError> {
        self.fullnode_store.get_source_head(subnet_id)
    }

    fn get_certificate(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<CertificateDelivered>, StorageError> {
        self.fullnode_store.get_certificate(certificate_id)
    }

    fn get_certificates(
        &self,
        certificate_ids: &[CertificateId],
    ) -> Result<Vec<Option<CertificateDelivered>>, StorageError> {
        self.fullnode_store.get_certificates(certificate_ids)
    }

    fn last_delivered_position_for_subnet(
        &self,
        subnet_id: &SubnetId,
    ) -> Result<Option<CertificateSourceStreamPosition>, StorageError> {
        Ok(self
            .fullnode_store
            .index_tables
            .source_list
            .get(subnet_id)?
            .map(|(_, position)| CertificateSourceStreamPosition {
                subnet_id: *subnet_id,
                position,
            }))
    }

    fn get_checkpoint(&self) -> Result<HashMap<SubnetId, SourceHead>, StorageError> {
        self.fullnode_store.get_checkpoint()
    }

    fn get_source_stream_certificates_from_position(
        &self,
        from: CertificateSourceStreamPosition,
        limit: usize,
    ) -> Result<Vec<(CertificateDelivered, CertificateSourceStreamPosition)>, StorageError> {
        self.fullnode_store
            .get_source_stream_certificates_from_position(from, limit)
    }

    fn get_target_stream_certificates_from_position(
        &self,
        position: CertificateTargetStreamPosition,
        limit: usize,
    ) -> Result<Vec<(CertificateDelivered, CertificateTargetStreamPosition)>, StorageError> {
        self.fullnode_store
            .get_target_stream_certificates_from_position(position, limit)
    }

    fn get_target_source_subnet_list(
        &self,
        target_subnet_id: &SubnetId,
    ) -> Result<Vec<SubnetId>, StorageError> {
        self.fullnode_store
            .get_target_source_subnet_list(target_subnet_id)
    }
}

#[async_trait]
impl WriteStore for ValidatorStore {
    async fn insert_certificate_delivered(
        &self,
        certificate: &CertificateDelivered,
    ) -> Result<CertificatePositions, StorageError> {
        let position = self
            .fullnode_store
            .insert_certificate_delivered(certificate)
            .await?;

        if let Ok(Some(pending_id)) = self
            .pending_tables
            .pending_pool_index
            .get(&certificate.certificate.id)
        {
            _ = self.pending_tables.pending_pool.delete(&pending_id);
            _ = self
                .pending_tables
                .pending_pool_index
                .delete(&certificate.certificate.id);

            STORAGE_PENDING_POOL_COUNT.dec();
        }

        if let Ok(Some(next_certificate)) = self
            .pending_tables
            .precedence_pool
            .get(&certificate.certificate.id)
        {
            debug!(
                "Delivered certificate {} unlocks {} for broadcast",
                certificate.certificate.id, next_certificate.id
            );
            self.insert_pending_certificate(&next_certificate).await?;
            self.pending_tables
                .precedence_pool
                .delete(&certificate.certificate.id)?;

            STORAGE_PRECEDENCE_POOL_COUNT.dec();
            STORAGE_PENDING_POOL_COUNT.inc();
        }

        Ok(position)
    }

    async fn insert_certificates_delivered(
        &self,
        certificates: &[CertificateDelivered],
    ) -> Result<(), StorageError> {
        self.fullnode_store
            .insert_certificates_delivered(certificates)
            .await
    }
}
