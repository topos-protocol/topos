use serde::{de::DeserializeOwned, Serialize};

pub trait Map<'a, K, V>
where
    K: Serialize + DeserializeOwned + ?Sized,
    V: Serialize + DeserializeOwned,
{
    type Iterator: Iterator<Item = (K, V)>;

    fn iter(&'a self) -> Self::Iterator;

    fn prefix_iter<P: Serialize>(&'a self, prefix: &P) -> Self::Iterator;
}
