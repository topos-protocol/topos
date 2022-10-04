use std::{marker::PhantomData, path::Path, sync::Arc};

use bincode::Options;
use rocksdb::{BoundColumnFamily, ColumnFamilyDescriptor, DBWithThreadMode, MultiThreaded};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tower::Service;

use crate::errors::InternalStorageError;

use super::RocksDBError;

#[derive(Debug)]
pub struct DBColumn<K, V> {
    pub db: Arc<DBWithThreadMode<MultiThreaded>>,
    _phantom: PhantomData<fn(K) -> V>,
    cf: String,
}

impl<K, V> DBColumn<K, V>
where
    K: DeserializeOwned + Serialize,
    V: DeserializeOwned + Serialize,
{
    pub fn open<P: AsRef<Path>>(
        path: P,
        db_options: Option<rocksdb::Options>,
        column: &str,
    ) -> Result<Self, InternalStorageError> {
        let mut options = db_options.unwrap_or_else(|| rocksdb::Options::default());
        let default_rocksdb_options = rocksdb::Options::default();

        let primary = path.as_ref().to_path_buf();

        let db = {
            options.create_if_missing(true);
            options.create_missing_column_families(true);
            Arc::new(
                rocksdb::DBWithThreadMode::<MultiThreaded>::open_cf_descriptors(
                    &options,
                    &primary,
                    vec![ColumnFamilyDescriptor::new(column, default_rocksdb_options)],
                )
                .unwrap(),
            )
        };

        Ok(Self {
            db,
            _phantom: PhantomData,
            cf: column.to_string(),
        })
    }

    fn cf(&self) -> Arc<BoundColumnFamily<'_>> {
        self.db.cf_handle(&self.cf).unwrap()
    }

    pub fn insert(&self, key: &K, value: &V) -> Result<(), InternalStorageError> {
        let key_buf = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding()
            .serialize(key)
            .unwrap();
        let value_buf = bincode::serialize(value).unwrap();

        self.db.put_cf(&self.cf(), &key_buf, &value_buf).unwrap();
        Ok(())
    }
}

impl Service<Insert> for DBColumn<K, V> {
    type Response = Result<(), InternalStorageError;

    type Error;

    type Future;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        todo!()
    }

    fn call(&mut self, req: Insert) -> Self::Future {
        todo!()
    }
}
