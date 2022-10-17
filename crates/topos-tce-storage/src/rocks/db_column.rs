use std::{marker::PhantomData, sync::Arc};

#[cfg(test)]
use std::path::Path;

use rocksdb::BoundColumnFamily;
#[cfg(test)]
use rocksdb::ColumnFamilyDescriptor;

use bincode::Options;
use serde::{de::DeserializeOwned, Serialize};

use crate::errors::InternalStorageError;

use super::{iterator::ColumnIterator, map::Map, RocksDB};

/// A DBColumn represents a CF structure
#[derive(Debug)]
pub struct DBColumn<K, V> {
    db: RocksDB,
    _phantom: PhantomData<fn(K) -> V>,
    cf: String,
}

impl<K, V> DBColumn<K, V>
where
    K: DeserializeOwned + Serialize,
    V: DeserializeOwned + Serialize,
{
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn open<P: AsRef<Path>>(
        path: P,
        db_options: Option<rocksdb::Options>,
        column: &str,
    ) -> Result<Self, InternalStorageError> {
        let mut options = db_options.unwrap_or_default();
        let default_rocksdb_options = rocksdb::Options::default();

        let primary = path.as_ref().to_path_buf();

        let db = {
            options.create_if_missing(true);
            options.create_missing_column_families(true);
            RocksDB {
                rocksdb: Arc::new(
                    rocksdb::DBWithThreadMode::<rocksdb::MultiThreaded>::open_cf_descriptors(
                        &options,
                        &primary,
                        vec![ColumnFamilyDescriptor::new(column, default_rocksdb_options)],
                    )
                    .unwrap(),
                ),
                batch_in_progress: Default::default(),
                atomic_batch: Default::default(),
            }
        };

        Ok(Self {
            db,
            _phantom: PhantomData,
            cf: column.to_string(),
        })
    }

    pub fn reopen(db: &RocksDB, column: &str) -> Self {
        Self {
            db: db.clone(),
            _phantom: PhantomData,
            cf: column.to_string(),
        }
    }

    /// Returns the CF of the DBColumn, used to build queries.
    fn cf(&self) -> Arc<BoundColumnFamily<'_>> {
        self.db.rocksdb.cf_handle(&self.cf).unwrap()
    }

    /// Insert a record into the storage by passing a Key and a Value.
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn insert(&self, key: &K, value: &V) -> Result<(), InternalStorageError> {
        let cf = self.cf();

        let key_buf = be_fix_int_ser(key).unwrap();

        let value_buf = bincode::serialize(value).unwrap();

        self.db.rocksdb.put_cf(&cf, &key_buf, &value_buf).unwrap();

        Ok(())
    }

    /// Delete a record from the storage by passing a Key
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn delete(&self, key: &K) -> Result<(), InternalStorageError> {
        let key_buf = be_fix_int_ser(key).unwrap();

        self.db.rocksdb.delete_cf(&self.cf(), key_buf).unwrap();

        Ok(())
    }

    /// Get a record from the storage by passing a Key
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn get(&self, key: &K) -> Result<V, InternalStorageError> {
        let key_buf = be_fix_int_ser(key).unwrap();

        self.db
            .rocksdb
            .get_pinned_cf(&self.cf(), key_buf)?
            .map(|data| bincode::deserialize(&data).unwrap())
            .ok_or(InternalStorageError::UnableToDeserializeValue)
    }
}

impl<'a, K, V> Map<'a, K, V> for DBColumn<K, V>
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    type Iterator = ColumnIterator<'a, K, V>;

    fn iter(&'a self) -> Self::Iterator {
        let mut raw_iterator = self.db.rocksdb.raw_iterator_cf(&self.cf());
        raw_iterator.seek_to_first();

        ColumnIterator::new(raw_iterator)
    }

    fn prefix_iter<P: Serialize>(&'a self, prefix: &P) -> Self::Iterator {
        let iterator = self
            .db
            .rocksdb
            .prefix_iterator_cf(&self.cf(), be_fix_int_ser(prefix).unwrap())
            .into();

        ColumnIterator::new(iterator)
    }
}

/// Serialize a value using a fix length serialize and a big endian endianness
fn be_fix_int_ser<S>(t: &S) -> Result<Vec<u8>, InternalStorageError>
where
    S: Serialize + ?Sized,
{
    Ok(bincode::DefaultOptions::new()
        .with_big_endian()
        .with_fixint_encoding()
        .serialize(t)?)
}
