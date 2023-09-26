use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};

use async_trait::async_trait;

use topos_core::{
    types::{
        stream::{CertificateSourceStreamPosition, Position},
        CertificateDelivered, ProofOfDelivery,
    },
    uci::{Certificate, CertificateId, SubnetId, INITIAL_CERTIFICATE_ID},
};
use tracing::{debug, info, instrument};

use crate::{
    errors::{InternalStorageError, StorageError},
    fullnode::FullNodeStore,
    rocks::map::Map,
    store::{ReadStore, WriteStore},
    CertificatePositions, CertificateTargetStreamPosition, PendingCertificateId, SourceHead,
};

pub(crate) use self::tables::ValidatorPendingTables;
pub use self::tables::ValidatorPerpetualTables;

mod tables;

/// Contains all persistent data about the validator
pub struct ValidatorStore {
    pub(crate) pending_tables: ValidatorPendingTables,
    pub(crate) fullnode_store: Arc<FullNodeStore>,
}

impl ValidatorStore {
    pub fn open(
        path: PathBuf,
        fullnode_store: Arc<FullNodeStore>,
    ) -> Result<Arc<Self>, StorageError> {
        let pending_tables: ValidatorPendingTables = ValidatorPendingTables::open(path);
        let store = Arc::new(Self {
            pending_tables,
            fullnode_store,
        });

        Ok(store)
    }

    pub fn get_fullnode_store(&self) -> Arc<FullNodeStore> {
        self.fullnode_store.clone()
    }

    pub fn count_pending_certificates(&self) -> Result<usize, StorageError> {
        Ok(self.pending_tables.pending_pool.iter()?.count())
    }

    pub fn get_pending_id(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<PendingCertificateId>, StorageError> {
        Ok(self.pending_tables.pending_pool_index.get(certificate_id)?)
    }

    pub fn get_pending_certificate(
        &self,
        pending_id: &PendingCertificateId,
    ) -> Result<Option<Certificate>, StorageError> {
        Ok(self.pending_tables.pending_pool.get(pending_id)?)
    }

    pub fn get_pending_certificates(
        &self,
    ) -> Result<Vec<(PendingCertificateId, Certificate)>, StorageError> {
        Ok(self.pending_tables.pending_pool.iter()?.collect())
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

    pub fn insert_pending_certificates(
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

        Ok(ids)
    }

    pub fn insert_pending_certificate(
        &self,
        certificate: &Certificate,
    ) -> Result<Option<PendingCertificateId>, StorageError> {
        if self.get_certificate(&certificate.id)?.is_some() {
            return Err(StorageError::InternalStorage(
                InternalStorageError::CertificateAlreadyExists,
            ));
        }

        let prev_delivered = certificate.prev_id == INITIAL_CERTIFICATE_ID
            || self
                .fullnode_store
                .get_certificate(&certificate.prev_id)?
                .is_some();

        if prev_delivered {
            let id = self
                .pending_tables
                .next_pending_id
                .fetch_add(1, Ordering::Relaxed);

            self.pending_tables.pending_pool.insert(&id, certificate)?;
            self.pending_tables
                .pending_pool_index
                .insert(&certificate.id, &id)?;

            Ok(Some(id))
        } else {
            self.pending_tables
                .precedence_pool
                .insert(&certificate.prev_id, certificate)?;

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
                "Certificate Sync: unverified proof as been removed for {}",
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

    pub fn get_checkpoint_diff(
        &self,
        from: Vec<ProofOfDelivery>,
    ) -> Result<HashMap<SubnetId, Vec<ProofOfDelivery>>, StorageError> {
        // Parse the from in order to extract the different position per subnets
        let mut from_positions: HashMap<SubnetId, Vec<ProofOfDelivery>> = from
            .into_iter()
            .map(|v| (v.delivery_position.subnet_id, vec![v]))
            .collect();

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
            let entry = from_positions.entry(subnet).or_default();

            let certs: Vec<_> = if let Some(position) = entry.pop() {
                if local_position <= position.delivery_position.position {
                    continue;
                }
                self.fullnode_store
                    .perpetual_tables
                    .streams
                    .prefix_iter_at(&subnet, &position)?
                    .take(100)
                    .map(|(_, v)| v)
                    .collect()
            } else {
                self.fullnode_store
                    .perpetual_tables
                    .streams
                    .prefix_iter(&subnet)?
                    .take(100)
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
            entry.extend_from_slice(&proofs[..]);
        }

        Ok(from_positions)
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
        }

        if let Ok(Some(certificate)) = self
            .pending_tables
            .precedence_pool
            .get(&certificate.certificate.id)
        {
            self.insert_pending_certificate(&certificate)?;
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
