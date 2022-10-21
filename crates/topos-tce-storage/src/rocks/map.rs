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

    /// Returns a prefixed Iterator over the CF
    fn prefix_iter<P: Serialize>(
        &'a self,
        prefix: &P,
    ) -> Result<Self::Iterator, InternalStorageError>;
}
