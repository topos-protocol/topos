use std::marker::PhantomData;

use bincode::Options;
use rocksdb::{DBRawIteratorWithThreadMode, DBWithThreadMode, Direction, MultiThreaded};
use serde::de::DeserializeOwned;

pub struct ColumnIterator<'a, K, V> {
    iterator: DBRawIteratorWithThreadMode<'a, DBWithThreadMode<MultiThreaded>>,
    direction: Direction,
    _phantom: PhantomData<(K, V)>,
}

impl<'a, K, V> ColumnIterator<'a, K, V> {
    /// Creates a new ColumnIterator base on a DBRawIteratorWithThreadMode
    pub fn new(iterator: DBRawIteratorWithThreadMode<'a, DBWithThreadMode<MultiThreaded>>) -> Self {
        Self::new_with_direction(iterator, Direction::Forward)
    }

    pub fn new_with_direction(
        iterator: DBRawIteratorWithThreadMode<'a, DBWithThreadMode<MultiThreaded>>,
        direction: Direction,
    ) -> Self {
        Self {
            iterator,
            direction,
            _phantom: PhantomData,
        }
    }
}

impl<'a, K, V> Iterator for ColumnIterator<'a, K, V>
where
    K: DeserializeOwned,
    V: DeserializeOwned,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        if self.iterator.valid() {
            let config = bincode::DefaultOptions::new()
                .with_big_endian()
                .with_fixint_encoding();

            let key = self.iterator.key().and_then(|k| config.deserialize(k).ok());
            let value = self
                .iterator
                .value()
                .and_then(|v| bincode::deserialize(v).ok());

            match self.direction {
                Direction::Forward => self.iterator.next(),
                Direction::Reverse => self.iterator.prev(),
            }

            key.and_then(|k| value.map(|v| (k, v)))
        } else {
            None
        }
    }
}
