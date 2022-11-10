use std::{
    fmt::Debug,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use topos_core::uci::{Certificate, CertificateId};
use tracing::warn;

use crate::{errors::InternalStorageError, Height, PendingCertificateId, Storage, SubnetId};

use self::{db::DB, db_column::DBColumn};
use self::{
    db::{init_db, RocksDB},
    map::Map,
};

pub(crate) mod constants;
pub(crate) mod db;
pub(crate) mod db_column;
pub(crate) mod iterator;
pub(crate) mod map;
mod types;

pub(crate) use types::*;

#[derive(Debug)]
pub struct RocksDBStorage {
    pending_certificates: PendingCertificatesColumn,
    certificates: CertificatesColumn,

    source_subnet_streams: SourceSubnetStreamsColumn,
    target_subnet_streams: TargetSubnetStreamsColumn,

    next_pending_id: AtomicU64,
}

impl RocksDBStorage {
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn new(
        pending_certificates: PendingCertificatesColumn,
        certificates: CertificatesColumn,
        source_subnet_streams: SourceSubnetStreamsColumn,
        target_subnet_streams: TargetSubnetStreamsColumn,
        next_pending_id: AtomicU64,
    ) -> Self {
        Self {
            pending_certificates,
            certificates,
            source_subnet_streams,
            target_subnet_streams,
            next_pending_id,
        }
    }

    pub fn open(path: &PathBuf) -> Result<Self, InternalStorageError> {
        let db = DB.get_or_try_init(|| {
            let mut options = rocksdb::Options::default();
            options.create_if_missing(true);
            options.create_missing_column_families(true);
            init_db(path, &mut options)
        })?;

        let pending_certificates = DBColumn::reopen(db, constants::PENDING_CERTIFICATES);

        let next_pending_id = match pending_certificates.iter()?.last() {
            Some((pending_id, _)) => AtomicU64::new(pending_id),
            None => AtomicU64::new(0),
        };

        Ok(Self {
            pending_certificates,
            certificates: DBColumn::reopen(db, constants::CERTIFICATES),
            source_subnet_streams: DBColumn::reopen(db, constants::SOURCE_SUBNET_STREAMS),
            target_subnet_streams: DBColumn::reopen(db, constants::TARGET_SUBNET_STREAMS),
            next_pending_id,
        })
    }
}

#[async_trait::async_trait]
impl Storage for RocksDBStorage {
    async fn add_pending_certificate(
        &self,
        certificate: Certificate,
    ) -> Result<PendingCertificateId, InternalStorageError> {
        let key = self.next_pending_id.fetch_add(1, Ordering::Relaxed);

        self.pending_certificates.insert(&key, &certificate)?;

        Ok(key)
    }

    async fn persist(
        &self,
        certificate: Certificate,
        pending_certificate_id: Option<PendingCertificateId>,
    ) -> Result<(), InternalStorageError> {
        let mut batch = self.certificates.batch();

        let source_subnet_id: SubnetId = certificate.initial_subnet_id.as_str().try_into()?;

        // Inserting the certificate data into the CERTIFICATES cf
        batch = batch.insert_batch(&self.certificates, [(&certificate.cert_id, &certificate)])?;

        if let Some(pending_id) = pending_certificate_id {
            match self.pending_certificates.get(&pending_id) {
                Ok(pending_certificate) if pending_certificate == certificate => {
                    batch = batch.delete(&self.pending_certificates, pending_id)?;
                }
                Ok(_) => {
                    warn!("PendingCertificateId {} ignored during persist execution: Difference in certificates", pending_id);
                }

                _ => {
                    warn!(
                        "PendingCertificateId {} ignored during persist execution: Not Found",
                        pending_id
                    );
                }
            }
        }

        let source_subnet_height = if certificate.prev_cert_id.is_empty() {
            Height::ZERO
        } else if let Some((SourceStreamRef(_, height), _)) = self
            .source_subnet_streams
            .prefix_iter(&source_subnet_id)?
            .last()
        {
            height.increment().map_err(|error| {
                InternalStorageError::HeightError(error, certificate.initial_subnet_id.clone())
            })?
        } else {
            // TODO: Better error to define that we were expecting a previous defined height
            return Err(InternalStorageError::CertificateNotFound(
                certificate.prev_cert_id,
            ));
        };

        // Adding the certificate to the stream
        batch = batch.insert_batch(
            &self.source_subnet_streams,
            [(
                SourceStreamRef(source_subnet_id, source_subnet_height),
                certificate.cert_id.clone(),
            )],
        )?;

        // Adding certificate to target_subnet_streams
        // TODO: Add expected version instead of calculating on the go
        let mut targets = Vec::new();

        for transaction in certificate.calls {
            let target = if let Some((TargetStreamRef(target, source, height), _)) = self
                .target_subnet_streams
                .prefix_iter(&TargetStreamPrefix(
                    transaction.terminal_subnet_id.as_str().try_into()?,
                    source_subnet_id,
                ))?
                .last()
            {
                (
                    TargetStreamRef(
                        target,
                        source,
                        height.increment().map_err(|error| {
                            InternalStorageError::HeightError(
                                error,
                                certificate.initial_subnet_id.clone(),
                            )
                        })?,
                    ),
                    certificate.cert_id.clone(),
                )
            } else {
                (
                    TargetStreamRef(
                        transaction.terminal_subnet_id.as_str().try_into()?,
                        source_subnet_id,
                        Height::ZERO,
                    ),
                    certificate.cert_id.clone(),
                )
            };

            targets.push(target);
        }

        batch = batch.insert_batch(&self.target_subnet_streams, targets)?;

        batch.write()?;

        Ok(())
    }

    async fn update(
        &self,
        _certificate_id: &CertificateId,
        _status: crate::CertificateStatus,
    ) -> Result<(), InternalStorageError> {
        unimplemented!();
    }

    async fn get_tip(
        &self,
        _subnets: Vec<SubnetId>,
    ) -> Result<Vec<crate::Tip>, InternalStorageError> {
        unimplemented!();
    }

    async fn get_certificates(
        &self,
        certificate_ids: Vec<CertificateId>,
    ) -> Result<Vec<Certificate>, InternalStorageError> {
        let mut result = Vec::new();

        for certificate_id in certificate_ids {
            result.push(self.get_certificate(certificate_id).await?);
        }

        Ok(result)
    }

    async fn get_certificate(
        &self,
        certificate_id: CertificateId,
    ) -> Result<Certificate, InternalStorageError> {
        self.certificates.get(&certificate_id)
    }

    async fn get_certificates_by_source(
        &self,
        _source_subnet_id: SubnetId,
        _from: crate::Height,
        _to: crate::Height,
    ) -> Result<Vec<CertificateId>, InternalStorageError> {
        unimplemented!();
    }

    async fn get_certificates_by_target(
        &self,
        _target_subnet_id: SubnetId,
        _from: std::time::Instant,
        _to: std::time::Instant,
    ) -> Result<Vec<CertificateId>, InternalStorageError> {
        unimplemented!()
    }

    async fn get_pending_certificates(
        &self,
    ) -> Result<Vec<(u64, Certificate)>, InternalStorageError> {
        Ok(self.pending_certificates.iter()?.collect())
    }

    async fn remove_pending_certificate(&self, index: u64) -> Result<(), InternalStorageError> {
        self.pending_certificates.delete(&index)
    }
}

#[cfg(test)]
impl RocksDBStorage {
    pub(crate) fn pending_certificates_column(&self) -> PendingCertificatesColumn {
        self.pending_certificates.clone()
    }

    pub(crate) fn certificates_column(&self) -> CertificatesColumn {
        self.certificates.clone()
    }

    pub(crate) fn source_subnet_streams_column(&self) -> SourceSubnetStreamsColumn {
        self.source_subnet_streams.clone()
    }

    pub(crate) fn target_subnet_streams_column(&self) -> TargetSubnetStreamsColumn {
        self.target_subnet_streams.clone()
    }
}
