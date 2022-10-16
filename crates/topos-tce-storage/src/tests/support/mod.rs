use std::{
    path::PathBuf,
    sync::{atomic::AtomicU64, Arc},
};

use rocksdb::Options;
use rstest::fixture;

use crate::{
    rocks::{
        CertificatesColumn, PendingCertificatesColumn, RocksDB, SourceSubnetStreamsColumn,
        TargetSubnetStreamsColumn,
    },
    RocksDBStorage,
};

use self::{
    columns::{certificates_column, pending_column, source_streams_column, target_streams_column},
    folder::created_folder,
};

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
pub(crate) fn rocksdb(created_folder: Box<PathBuf>) -> RocksDB {
    let mut options = Options::default();
    options.create_if_missing(true);
    RocksDB {
        rocksdb: Arc::new(rocksdb::DB::open(&options, created_folder)),
        batch_in_progress: Default::default(),
        atomic_batch: Default::default(),
    }
}
