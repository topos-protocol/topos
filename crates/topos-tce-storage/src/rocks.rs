use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{
    errors::{InternalStorageError, StorageError},
    Storage,
};

use self::db_column::{DBColumn, Map};

pub(crate) mod constants;
pub(crate) mod db_column;
mod iterator;

pub(crate) type SourceStreamRef = (SubnetId, u64);
pub(crate) type TargetStreamRef = (SubnetId, SubnetId, u64);
pub(crate) type PendingCertificatesColumn = DBColumn<u64, Certificate>;
pub(crate) type CertificatesColumn = DBColumn<CertificateId, Certificate>;
pub(crate) type SourceSubnetStreamsColumn = DBColumn<SourceStreamRef, CertificateId>;
pub(crate) type TargetSubnetStreamsColumn = DBColumn<TargetStreamRef, CertificateId>;

#[derive(Debug)]
pub enum RocksDBError {}

#[derive(Debug)]
pub struct RocksDBStorage {
    pending_certificates: PendingCertificatesColumn,
    certificates: CertificatesColumn,
    #[allow(dead_code)]
    source_subnet_streams: SourceSubnetStreamsColumn,
    #[allow(dead_code)]
    target_subnet_streams: TargetSubnetStreamsColumn,

    next_pending_id: AtomicU64,
}

impl RocksDBStorage {
    pub async fn open(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            pending_certificates: DBColumn::open(path, None, constants::PENDING_CERTIFICATES)
                .unwrap(),
            certificates: DBColumn::open(path, None, constants::CERTIFICATES).unwrap(),
            source_subnet_streams: DBColumn::open(path, None, constants::SOURCE_SUBNET_STREAMS)
                .unwrap(),
            target_subnet_streams: DBColumn::open(path, None, constants::TARGET_SUBNET_STREAMS)
                .unwrap(),
            next_pending_id: AtomicU64::new(0),
        })
    }
}

#[async_trait::async_trait]
impl Storage for RocksDBStorage {
    async fn add_pending_certificate(
        &mut self,
        certificate: Certificate,
    ) -> Result<(), InternalStorageError> {
        let key = self.next_pending_id.fetch_add(1, Ordering::Relaxed);

        self.pending_certificates.insert(&key, &certificate)
    }

    async fn persist(
        &mut self,
        _certificate: Certificate,
        _status: crate::CertificateStatus,
    ) -> Result<(), InternalStorageError> {
        unimplemented!();
    }

    async fn update(
        &mut self,
        _certificate_id: &CertificateId,
        _status: crate::CertificateStatus,
    ) -> Result<(), InternalStorageError> {
        unimplemented!();
    }

    async fn get_tip(
        &self,
        _subnets: Vec<topos_core::uci::SubnetId>,
    ) -> Result<Vec<crate::Tip>, InternalStorageError> {
        unimplemented!();
    }

    async fn get_certificates(
        &self,
        _cert_id: Vec<CertificateId>,
    ) -> Result<Vec<(crate::CertificateStatus, Certificate)>, InternalStorageError> {
        unimplemented!();
    }

    async fn get_certificate(
        &self,
        cert_id: CertificateId,
    ) -> Result<Certificate, InternalStorageError> {
        self.certificates.get(&cert_id)
    }

    async fn get_emitted_certificates(
        &self,
        _subnet_id: topos_core::uci::SubnetId,
        _from: crate::Height,
        _to: crate::Height,
    ) -> Result<Vec<CertificateId>, InternalStorageError> {
        unimplemented!();
    }

    async fn get_received_certificates(
        &self,
        _subnet_id: topos_core::uci::SubnetId,
        _from: std::time::Instant,
        _to: std::time::Instant,
    ) -> Result<Vec<CertificateId>, InternalStorageError> {
        unimplemented!()
    }

    async fn get_pending_certificates(
        &self,
    ) -> Result<Vec<(u64, Certificate)>, InternalStorageError> {
        Ok(self.pending_certificates.iter().collect())
    }

    async fn remove_pending_certificate(&self, index: u64) -> Result<(), InternalStorageError> {
        self.pending_certificates.delete(&index)
    }
}
