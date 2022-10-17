use serde::{de::DeserializeOwned, Serialize};

pub trait Map<'a, K, V>
where
    K: Serialize + DeserializeOwned + ?Sized,
    V: Serialize + DeserializeOwned,
{
    type Iterator: Iterator<Item = (K, V)>;

    /// Returns an Iterator over the whole CF
    fn iter(&'a self) -> Self::Iterator;

    /// Returns a prefixed Iterator over the CF
    fn prefix_iter<P: Serialize>(&'a self, prefix: &P) -> Self::Iterator;
}
