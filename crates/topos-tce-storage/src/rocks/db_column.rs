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
    cf: &'static str,
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
        column: &'static str,
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
                    )?,
                ),
                batch_in_progress: Default::default(),
                atomic_batch: Default::default(),
            }
        };

        Ok(Self {
            db,
            _phantom: PhantomData,
            cf: column,
        })
    }

    pub fn reopen(db: &RocksDB, column: &'static str) -> Self {
        Self {
            db: db.clone(),
            _phantom: PhantomData,
            cf: column,
        }
    }

    /// Returns the CF of the DBColumn, used to build queries.
    fn cf(&self) -> Result<Arc<BoundColumnFamily<'_>>, InternalStorageError> {
        self.db
            .rocksdb
            .cf_handle(self.cf)
            .ok_or(InternalStorageError::InvalidColumnFamily(self.cf))
    }

    /// Insert a record into the storage by passing a Key and a Value.
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn insert(&self, key: &K, value: &V) -> Result<(), InternalStorageError> {
        let cf = self.cf()?;

        let key_buf = be_fix_int_ser(key)?;

        let value_buf = bincode::serialize(value)?;

        self.db.rocksdb.put_cf(&cf, &key_buf, &value_buf)?;

        Ok(())
    }

    /// Delete a record from the storage by passing a Key
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn delete(&self, key: &K) -> Result<(), InternalStorageError> {
        let key_buf = be_fix_int_ser(key)?;

        self.db.rocksdb.delete_cf(&self.cf()?, key_buf)?;

        Ok(())
    }

    /// Get a record from the storage by passing a Key
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn get(&self, key: &K) -> Result<V, InternalStorageError> {
        let key_buf = be_fix_int_ser(key)?;

        let data = self
            .db
            .rocksdb
            .get_pinned_cf(&self.cf()?, key_buf)?
            .ok_or(InternalStorageError::UnableToDeserializeValue)?;

        Ok(bincode::deserialize(&data)?)
    }
}

impl<'a, K, V> Map<'a, K, V> for DBColumn<K, V>
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    type Iterator = ColumnIterator<'a, K, V>;

    fn iter(&'a self) -> Result<Self::Iterator, InternalStorageError> {
        let mut raw_iterator = self.db.rocksdb.raw_iterator_cf(&self.cf()?);
        raw_iterator.seek_to_first();

        Ok(ColumnIterator::new(raw_iterator))
    }

    fn prefix_iter<P: Serialize>(
        &'a self,
        prefix: &P,
    ) -> Result<Self::Iterator, InternalStorageError> {
        let iterator = self
            .db
            .rocksdb
            .prefix_iterator_cf(&self.cf()?, be_fix_int_ser(prefix)?)
            .into();

        Ok(ColumnIterator::new(iterator))
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
