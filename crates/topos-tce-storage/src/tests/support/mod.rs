use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

use once_cell::sync::Lazy;
use rocksdb::Options;
use rstest::fixture;

use crate::{
    rocks::{
        db::init_db, db::RocksDB, CertificatesColumn, PendingCertificatesColumn,
        SourceStreamsColumn, TargetStreamsColumn,
    },
    RocksDBStorage, SubnetId,
};

use self::{
    columns::{certificates_column, pending_column, source_streams_column, target_streams_column},
    folder::created_folder,
};

pub(crate) const SOURCE_SUBNET_ID: SubnetId = SubnetId { inner: [1u8; 32] };
pub(crate) const TARGET_SUBNET_ID_A: SubnetId = SubnetId { inner: [2u8; 32] };
pub(crate) const TARGET_SUBNET_ID_B: SubnetId = SubnetId { inner: [3u8; 32] };

pub(crate) mod columns;
pub(crate) mod folder;

pub(crate) static DB: Lazy<Mutex<HashMap<&'static str, Arc<RocksDB>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[fixture]
pub(crate) fn database_name() -> &'static str {
    Box::leak(Box::new(format!(
        "tests/databases/{}",
        uuid::Uuid::new_v4()
    )))
}

#[fixture]
pub(crate) fn storage(database_name: &'static str) -> RocksDBStorage {
    let pending_column: PendingCertificatesColumn = pending_column(database_name);
    let certificates_column: CertificatesColumn = certificates_column(database_name);
    let source_streams_column: SourceStreamsColumn = source_streams_column(database_name);
    let target_streams_column: TargetStreamsColumn = target_streams_column(database_name);

    RocksDBStorage::new(
        pending_column,
        certificates_column,
        source_streams_column,
        target_streams_column,
        AtomicU64::new(0),
    )
}

#[fixture]
pub(crate) fn rocks_db(database_name: &'static str) -> Arc<RocksDB> {
    let mut dbs = DB.lock().unwrap();

    dbs.entry(database_name)
        .or_insert_with(|| {
            let path = PathBuf::from_str(database_name).unwrap();
            created_folder(&path);
            let mut options = Options::default();
            options.create_if_missing(true);
            options.create_missing_column_families(true);

            Arc::new(init_db(&path, &mut options).unwrap())
        })
        .clone()
}
