use std::{
    fmt::Debug,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use rocksdb::{ColumnFamilyDescriptor, MultiThreaded};
use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{
    errors::{InternalStorageError, StorageError},
    PendingCertificateId, Storage,
};

use self::{db::RocksDB, map::Map};
use self::{db::DB, db_column::DBColumn};

pub(crate) mod constants;
pub(crate) mod db;
pub(crate) mod db_column;
pub(crate) mod iterator;
pub(crate) mod map;

pub(crate) type SourceStreamRef = (SubnetId, u64);
pub(crate) type TargetStreamRef = (SubnetId, SubnetId, u64);
pub(crate) type PendingCertificatesColumn = DBColumn<u64, Certificate>;
pub(crate) type CertificatesColumn = DBColumn<CertificateId, Certificate>;
pub(crate) type SourceSubnetStreamsColumn = DBColumn<SourceStreamRef, CertificateId>;
pub(crate) type TargetSubnetStreamsColumn = DBColumn<TargetStreamRef, CertificateId>;

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
    #[cfg(test)]
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

    pub async fn open(path: &PathBuf) -> Result<Self, StorageError> {
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let default_rocksdb_options = rocksdb::Options::default();

        let db = DB.get_or_try_init(|| {
            Ok::<_, StorageError>(RocksDB {
                rocksdb: Arc::new(
                    rocksdb::DBWithThreadMode::<MultiThreaded>::open_cf_descriptors(
                        &options,
                        &path,
                        vec![
                            ColumnFamilyDescriptor::new(
                                constants::PENDING_CERTIFICATES,
                                default_rocksdb_options.clone(),
                            ),
                            ColumnFamilyDescriptor::new(
                                constants::CERTIFICATES,
                                default_rocksdb_options.clone(),
                            ),
                            ColumnFamilyDescriptor::new(
                                constants::SOURCE_SUBNET_STREAMS,
                                default_rocksdb_options.clone(),
                            ),
                            ColumnFamilyDescriptor::new(
                                constants::TARGET_SUBNET_STREAMS,
                                default_rocksdb_options,
                            ),
                        ],
                    )
                    .unwrap(),
                ),
                batch_in_progress: Default::default(),
                atomic_batch: Default::default(),
            })
        })?;

        Ok(Self {
            pending_certificates: DBColumn::reopen(db, constants::PENDING_CERTIFICATES),
            certificates: DBColumn::reopen(db, constants::CERTIFICATES),
            source_subnet_streams: DBColumn::reopen(db, constants::SOURCE_SUBNET_STREAMS),
            target_subnet_streams: DBColumn::reopen(db, constants::TARGET_SUBNET_STREAMS),
            next_pending_id: AtomicU64::new(0),
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
        _certificate: Certificate,
        _status: crate::CertificateStatus,
    ) -> Result<PendingCertificateId, InternalStorageError> {
        unimplemented!();
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
        _subnets: Vec<topos_core::uci::SubnetId>,
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
