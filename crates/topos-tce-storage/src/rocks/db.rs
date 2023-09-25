use rocksdb::MultiThreaded;
use std::{path::PathBuf, sync::Arc};

use rocksdb::{ColumnFamilyDescriptor, Options};

use crate::errors::InternalStorageError;

use super::constants;

pub(crate) type RocksDB = Arc<rocksdb::DBWithThreadMode<MultiThreaded>>;

pub(crate) fn init_with_cfs(
    path: &PathBuf,
    mut options: rocksdb::Options,
    cfs: Vec<ColumnFamilyDescriptor>,
) -> Result<RocksDB, InternalStorageError> {
    options.create_missing_column_families(true);

    Ok(Arc::new(
        rocksdb::DBWithThreadMode::<MultiThreaded>::open_cf_descriptors(&options, path, cfs)?,
    ))
}
pub(crate) fn default_options() -> rocksdb::Options {
    let mut options = Options::default();
    options.create_if_missing(true);

    options
}

pub(crate) fn init_db(
    path: &PathBuf,
    options: rocksdb::Options,
) -> Result<RocksDB, InternalStorageError> {
    let mut options_source = default_options();
    options_source.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(
        constants::SOURCE_STREAMS_PREFIX_SIZE,
    ));

    let mut options_target = Options::default();
    options_target.create_if_missing(true);
    options_target.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(
        constants::TARGET_STREAMS_PREFIX_SIZE,
    ));
    let cfs = vec![
        ColumnFamilyDescriptor::new(constants::PENDING_CERTIFICATES, default_options()),
        ColumnFamilyDescriptor::new(constants::CERTIFICATES, rocksdb::Options::default()),
        ColumnFamilyDescriptor::new(constants::SOURCE_STREAMS, options_source),
        ColumnFamilyDescriptor::new(constants::TARGET_STREAMS, options_target),
        ColumnFamilyDescriptor::new(constants::TARGET_SOURCES, default_options()),
    ];

    init_with_cfs(path, options, cfs)
}
