use std::{
    path::PathBuf,
    sync::{atomic::AtomicU64, Arc},
};

use rocksdb::MultiThreaded;
use rocksdb::Options;
use rstest::fixture;

use crate::{
    rocks::{
        db::RocksDB, CertificatesColumn, PendingCertificatesColumn, SourceSubnetStreamsColumn,
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
pub(crate) fn rocks_db(created_folder: Box<PathBuf>) -> RocksDB {
    let mut options = Options::default();
    options.create_if_missing(true);
    RocksDB {
        rocksdb: Arc::new(
            rocksdb::DBWithThreadMode::<MultiThreaded>::open(&options, *created_folder).unwrap(),
        ),
        batch_in_progress: Default::default(),
        atomic_batch: Default::default(),
    }
}
