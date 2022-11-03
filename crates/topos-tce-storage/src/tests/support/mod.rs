use std::{path::PathBuf, sync::atomic::AtomicU64};

use rocksdb::Options;
use rstest::fixture;

use crate::{
    rocks::{
        db::init_db, db::RocksDB, CertificatesColumn, PendingCertificatesColumn,
        SourceSubnetStreamsColumn, TargetSubnetStreamsColumn,
    },
    RocksDBStorage, SubnetId,
};

use self::{
    columns::{certificates_column, pending_column, source_streams_column, target_streams_column},
    folder::created_folder,
};

pub(crate) const INITIAL_SUBNET_ID: SubnetId = SubnetId { inner: [1u8; 32] };
pub(crate) const TARGET_SUBNET_ID_A: SubnetId = SubnetId { inner: [2u8; 32] };
pub(crate) const TARGET_SUBNET_ID_B: SubnetId = SubnetId { inner: [3u8; 32] };

pub(crate) mod columns;
pub(crate) mod folder;

#[fixture]
pub(crate) fn storage(
    pending_column: PendingCertificatesColumn,
    certificates_column: CertificatesColumn,
    source_streams_column: SourceSubnetStreamsColumn,
    target_streams_column: TargetSubnetStreamsColumn,
) -> RocksDBStorage {
    RocksDBStorage::new(
        pending_column,
        certificates_column,
        source_streams_column,
        target_streams_column,
        AtomicU64::new(0),
    )
}

#[fixture]
#[once]
pub(crate) fn rocks_db(created_folder: Box<PathBuf>) -> RocksDB {
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    init_db(&created_folder, &mut options).unwrap()
}
