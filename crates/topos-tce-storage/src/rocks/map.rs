use rocksdb::IteratorMode;
use serde::{de::DeserializeOwned, Serialize};

use crate::errors::InternalStorageError;

pub trait Map<'a, K, V>
where
    K: Serialize + DeserializeOwned + ?Sized,
    V: Serialize + DeserializeOwned,
{
    type Iterator: Iterator<Item = (K, V)>;

    /// Returns an Iterator over the whole CF
    fn iter(&'a self) -> Result<Self::Iterator, InternalStorageError>;

    /// Returns an Iterator over the whole CF with mode configured
    fn iter_with_mode(
        &'a self,
        mode: IteratorMode<'_>,
    ) -> Result<Self::Iterator, InternalStorageError>;

    /// Returns a prefixed Iterator over the CF
    fn prefix_iter<P: Serialize>(
        &'a self,
        prefix: &P,
    ) -> Result<Self::Iterator, InternalStorageError>;

    /// Returns a prefixed Iterator over the CF starting from index
    fn prefix_iter_at<P: Serialize, I: Serialize>(
        &'a self,
        prefix: &P,
        index: &I,
    ) -> Result<Self::Iterator, InternalStorageError>;
}
