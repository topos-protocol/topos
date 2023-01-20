use std::{
    fmt::Debug,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use topos_core::uci::{Certificate, CertificateId};
use tracing::warn;

use crate::{
    errors::InternalStorageError, PendingCertificateId, Position, SourceHead, Storage, SubnetId,
};

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

const EMPTY_PREVIOUS_CERT_ID: [u8; 32] = [0u8; 32];

#[derive(Debug)]
pub struct RocksDBStorage {
    pending_certificates: PendingCertificatesColumn,
    certificates: CertificatesColumn,

    source_streams: SourceStreamsColumn,
    target_streams: TargetStreamsColumn,

    next_pending_id: AtomicU64,
}

impl RocksDBStorage {
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn new(
        pending_certificates: PendingCertificatesColumn,
        certificates: CertificatesColumn,
        source_streams: SourceStreamsColumn,
        target_streams: TargetStreamsColumn,
        next_pending_id: AtomicU64,
    ) -> Self {
        Self {
            pending_certificates,
            certificates,
            source_streams,
            target_streams,
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
            source_streams: DBColumn::reopen(db, constants::SOURCE_STREAMS),
            target_streams: DBColumn::reopen(db, constants::TARGET_STREAMS),
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
        certificate: &Certificate,
        pending_certificate_id: Option<PendingCertificateId>,
    ) -> Result<(), InternalStorageError> {
        let mut batch = self.certificates.batch();

        let source_subnet_id: SubnetId = certificate
            .source_subnet_id
            .try_into()
            .map_err(|_| InternalStorageError::InvalidSubnetId)?;

        // Inserting the certificate data into the CERTIFICATES cf
        batch = batch.insert_batch(&self.certificates, [(&certificate.id, certificate)])?;

        if let Some(pending_id) = pending_certificate_id {
            match self.pending_certificates.get(&pending_id) {
                Ok(ref pending_certificate) if pending_certificate == certificate => {
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

        let source_subnet_position = if certificate.prev_id.as_array() == &EMPTY_PREVIOUS_CERT_ID {
            Position::ZERO
        } else if let Some((SourceStreamPosition(_, position), _)) =
            self.source_streams.prefix_iter(&source_subnet_id)?.last()
        {
            position.increment().map_err(|error| {
                InternalStorageError::PositionError(error, certificate.source_subnet_id)
            })?
        } else {
            // TODO: Better error to define that we were expecting a previous defined position
            return Err(InternalStorageError::CertificateNotFound(
                certificate.prev_id,
            ));
        };

        // Adding the certificate to the stream
        batch = batch.insert_batch(
            &self.source_streams,
            [(
                SourceStreamPosition(source_subnet_id, source_subnet_position),
                certificate.id,
            )],
        )?;

        // Adding certificate to target_streams
        // TODO: Add expected version instead of calculating on the go
        let mut targets = Vec::new();

        for target_subnet_id in &certificate.target_subnets {
            let target = if let Some((TargetStreamPosition(target, source, position), _)) = self
                .target_streams
                .prefix_iter(&TargetStreamPrefix(
                    (*target_subnet_id)
                        .try_into()
                        .map_err(|_| InternalStorageError::InvalidSubnetId)?,
                    source_subnet_id,
                ))?
                .last()
            {
                (
                    TargetStreamPosition(
                        target,
                        source,
                        position.increment().map_err(|error| {
                            InternalStorageError::PositionError(error, certificate.source_subnet_id)
                        })?,
                    ),
                    certificate.id,
                )
            } else {
                (
                    TargetStreamPosition(
                        (*target_subnet_id)
                            .try_into()
                            .map_err(|_| InternalStorageError::InvalidSubnetId)?,
                        source_subnet_id,
                        Position::ZERO,
                    ),
                    certificate.id,
                )
            };

            targets.push(target);
        }

        batch = batch.insert_batch(&self.target_streams, targets)?;

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

    async fn get_source_heads(
        &self,
        subnets: Vec<SubnetId>,
    ) -> Result<Vec<crate::SourceHead>, InternalStorageError> {
        let mut result: Vec<crate::SourceHead> = Vec::new();
        for source_subnet_id in subnets {
            let (position, cert_id) = self
                .source_streams
                .prefix_iter(&source_subnet_id)?
                .last()
                .map(|(source_stream_position, cert_id)| (source_stream_position.1, cert_id))
                .ok_or(InternalStorageError::UnableToFindHeadForSubnet(
                    source_subnet_id,
                ))?;
            result.push(SourceHead {
                position,
                cert_id,
                subnet_id: source_subnet_id,
            });
        }
        Ok(result)
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
        source_subnet_id: SubnetId,
        from: crate::Position,
        limit: usize,
    ) -> Result<Vec<CertificateId>, InternalStorageError> {
        Ok(self
            .source_streams
            .prefix_iter(&source_subnet_id)?
            // TODO: Find a better way to convert u64 to usize
            .skip(from.0.try_into().unwrap())
            .take(limit)
            .map(|(_, certificate_id)| certificate_id)
            .collect())
    }

    async fn get_certificates_by_target(
        &self,
        target_subnet_id: SubnetId,
        source_subnet_id: SubnetId,
        from: Position,
        limit: usize,
    ) -> Result<Vec<CertificateId>, InternalStorageError> {
        Ok(self
            .target_streams
            .prefix_iter(&(&target_subnet_id, &source_subnet_id))?
            // TODO: Find a better way to convert u64 to usize
            .skip(from.0.try_into().unwrap())
            .take(limit)
            .map(|(_, certificate_id)| certificate_id)
            .collect())
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

    pub(crate) fn source_streams_column(&self) -> SourceStreamsColumn {
        self.source_streams.clone()
    }

    pub(crate) fn target_streams_column(&self) -> TargetStreamsColumn {
        self.target_streams.clone()
    }
}
