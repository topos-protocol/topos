use once_cell::sync::OnceCell;
use rocksdb::MultiThreaded;
use std::{path::PathBuf, sync::Arc};

use rocksdb::{ColumnFamilyDescriptor, Options};

use crate::errors::InternalStorageError;

use super::constants;

pub(crate) static DB: OnceCell<RocksDB> = OnceCell::new();
pub(crate) type RocksDB = Arc<rocksdb::DBWithThreadMode<MultiThreaded>>;

pub(crate) fn init_db(
    path: &PathBuf,
    options: &mut rocksdb::Options,
) -> Result<RocksDB, InternalStorageError> {
    let default_rocksdb_options = rocksdb::Options::default();

    let mut options_source = Options::default();
    options_source.create_if_missing(true);
    options_source.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(
        constants::SOURCE_SUBNET_STREAMS_PREFIX_SIZE,
    ));

    let mut options_target = Options::default();
    options_target.create_if_missing(true);
    options_target.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(
        constants::TARGET_SUBNET_STREAMS_PREFIX_SIZE,
    ));

    Ok(Arc::new(
        rocksdb::DBWithThreadMode::<MultiThreaded>::open_cf_descriptors(
            options,
            path,
            vec![
                ColumnFamilyDescriptor::new(
                    constants::PENDING_CERTIFICATES,
                    default_rocksdb_options.clone(),
                ),
                ColumnFamilyDescriptor::new(constants::CERTIFICATES, default_rocksdb_options),
                ColumnFamilyDescriptor::new(constants::SOURCE_SUBNET_STREAMS, options_source),
                ColumnFamilyDescriptor::new(constants::TARGET_SUBNET_STREAMS, options_target),
            ],
        )?,
    ))
}
