use std::{borrow::Borrow, marker::PhantomData, sync::Arc};

#[cfg(test)]
use std::path::Path;

#[cfg(test)]
use rocksdb::ColumnFamilyDescriptor;
use rocksdb::{
    BoundColumnFamily, DBRawIteratorWithThreadMode, DBWithThreadMode, MultiThreaded, ReadOptions,
    WriteBatch,
};

use bincode::Options;
use serde::{de::DeserializeOwned, Serialize};

use crate::errors::InternalStorageError;

use super::{iterator::ColumnIterator, map::Map, RocksDB};

/// A DBColumn represents a CF structure
#[derive(Clone, Debug)]
pub struct DBColumn<K, V> {
    pub(crate) rocksdb: RocksDB,
    _phantom: PhantomData<fn(K) -> V>,
    cf: &'static str,
}

impl<K, V> DBColumn<K, V> {
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

        let rocksdb = {
            options.create_if_missing(true);
            options.create_missing_column_families(true);
            Arc::new(
                rocksdb::DBWithThreadMode::<rocksdb::MultiThreaded>::open_cf_descriptors(
                    &options,
                    primary,
                    vec![ColumnFamilyDescriptor::new(column, default_rocksdb_options)],
                )?,
            )
        };

        Ok(Self {
            rocksdb,
            _phantom: PhantomData,
            cf: column,
        })
    }

    pub fn reopen(db: &RocksDB, column: &'static str) -> Self {
        Self {
            rocksdb: db.clone(),
            _phantom: PhantomData,
            cf: column,
        }
    }

    /// Returns the CF of the DBColumn, used to build queries.
    fn cf(&self) -> Result<Arc<BoundColumnFamily<'_>>, InternalStorageError> {
        self.rocksdb
            .cf_handle(self.cf)
            .ok_or(InternalStorageError::InvalidColumnFamily(self.cf))
    }
}

impl<K, V> DBColumn<K, V>
where
    K: DeserializeOwned + Serialize + std::fmt::Debug,
    V: DeserializeOwned + Serialize + std::fmt::Debug,
{
    /// Insert a record into the storage by passing a Key and a Value.
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn insert(&self, key: &K, value: &V) -> Result<(), InternalStorageError> {
        let cf = self.cf()?;

        let key_buf = be_fix_int_ser(key)?;

        let value_buf = bincode::serialize(value)?;

        self.rocksdb.put_cf(&cf, key_buf, value_buf)?;

        Ok(())
    }

    /// Delete a record from the storage by passing a Key
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn delete(&self, key: &K) -> Result<(), InternalStorageError> {
        let key_buf = be_fix_int_ser(key)?;

        self.rocksdb.delete_cf(&self.cf()?, key_buf)?;

        Ok(())
    }

    /// Get a record from the storage by passing a Key
    ///
    /// Key are fixed length bincode serialized.
    pub(crate) fn get(&self, key: &K) -> Result<Option<V>, InternalStorageError> {
        let key_buf = be_fix_int_ser(key)?;

        self.rocksdb
            .get_pinned_cf(&self.cf()?, key_buf)?
            .map_or(Ok(None), |v| {
                bincode::deserialize::<V>(&v)
                    .map(|r| Some(r))
                    .map_err(|_| InternalStorageError::UnableToDeserializeValue)
            })
    }

    #[allow(unused)]
    pub(crate) fn multi_insert(
        &self,
        key_value_pairs: impl IntoIterator<Item = (K, V)>,
    ) -> Result<(), InternalStorageError> {
        let batch = self.batch();

        batch.insert_batch(self, key_value_pairs)?.write()
    }

    pub(crate) fn multi_get(&self, keys: &[K]) -> Result<Vec<Option<V>>, InternalStorageError> {
        let keys: Result<Vec<_>, InternalStorageError> =
            keys.iter().map(|k| be_fix_int_ser(k.borrow())).collect();

        let results: Result<Vec<_>, InternalStorageError> = self
            .rocksdb
            .batched_multi_get_cf_opt(&self.cf()?, keys?, false, &ReadOptions::default())
            .into_iter()
            .map(|r| r.map_err(InternalStorageError::RocksDBError))
            .collect();

        results?
            .into_iter()
            .map(|e| match e {
                Some(v) => bincode::deserialize(&v)
                    .map_err(InternalStorageError::Bincode)
                    .map(|v| Some(v)),
                None => Ok(None),
            })
            .collect()
    }

    pub(crate) fn batch(&self) -> DBBatch {
        DBBatch::new(&self.rocksdb)
    }
}

pub(crate) struct DBBatch {
    rocksdb: Arc<DBWithThreadMode<MultiThreaded>>,
    batch: WriteBatch,
}

impl DBBatch {
    fn new(rocksdb: &Arc<DBWithThreadMode<MultiThreaded>>) -> Self {
        Self {
            rocksdb: rocksdb.clone(),
            batch: WriteBatch::default(),
        }
    }

    pub(crate) fn delete<K, V, Key>(
        mut self,
        db: &DBColumn<K, V>,
        key: Key,
    ) -> Result<Self, InternalStorageError>
    where
        K: Serialize,
        V: Serialize,
        Key: Borrow<K>,
    {
        check_cross_batch(&self.rocksdb, &db.rocksdb)?;

        let key_buffer = be_fix_int_ser(key.borrow())?;
        self.batch.delete_cf(&db.cf()?, key_buffer);

        Ok(self)
    }

    pub(crate) fn insert_batch<K, V, Key, Value>(
        mut self,
        db: &DBColumn<K, V>,
        values: impl IntoIterator<Item = (Key, Value)>,
    ) -> Result<Self, InternalStorageError>
    where
        K: Serialize + std::fmt::Debug,
        V: Serialize + std::fmt::Debug,
        Key: Borrow<K>,
        Value: Borrow<V>,
    {
        check_cross_batch(&self.rocksdb, &db.rocksdb)?;

        values
            .into_iter()
            .try_for_each::<_, Result<(), InternalStorageError>>(|(k, v)| {
                let key_buffer = be_fix_int_ser(k.borrow())?;
                let value_buffer = bincode::serialize(v.borrow())?;
                self.batch.put_cf(&db.cf()?, key_buffer, value_buffer);
                Ok(())
            })?;

        Ok(self)
    }

    pub(crate) fn write(self) -> Result<(), InternalStorageError> {
        self.rocksdb.write(self.batch)?;

        Ok(())
    }
}

impl<'a, K, V> Map<'a, K, V> for DBColumn<K, V>
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    type Iterator = ColumnIterator<'a, K, V>;

    fn iter(&'a self) -> Result<Self::Iterator, InternalStorageError> {
        let mut raw_iterator = self.rocksdb.raw_iterator_cf(&self.cf()?);
        raw_iterator.seek_to_first();

        Ok(ColumnIterator::new(raw_iterator))
    }

    fn prefix_iter<P: Serialize>(
        &'a self,
        prefix: &P,
    ) -> Result<Self::Iterator, InternalStorageError> {
        let iterator = self
            .rocksdb
            .prefix_iterator_cf(&self.cf()?, be_fix_int_ser(prefix)?)
            .into();

        Ok(ColumnIterator::new(iterator))
    }

    fn prefix_iter_at<P: Serialize, I: Serialize>(
        &'a self,
        prefix: &P,
        index: &I,
    ) -> Result<Self::Iterator, InternalStorageError> {
        let mut iterator: DBRawIteratorWithThreadMode<_> = self
            .rocksdb
            .prefix_iterator_cf(&self.cf()?, be_fix_int_ser(prefix)?)
            .into();

        iterator.seek(be_fix_int_ser(index)?);
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

fn check_cross_batch(base: &RocksDB, current: &RocksDB) -> Result<(), InternalStorageError> {
    if !Arc::ptr_eq(base, current) {
        return Err(InternalStorageError::ConcurrentDBBatchDetected);
    }

    Ok(())
}
