use std::{
    collections::{hash_map::RandomState, HashMap},
    hash::{BuildHasher, Hash, Hasher},
    sync::Arc,
};

use tokio::sync::{Mutex, RwLock};

const LOCK_SHARDING: usize = 2048;

type LocksVec<T> = Vec<RwLock<HashMap<T, Arc<Mutex<()>>>>>;

pub(crate) struct LockGuards<T: Hash + Eq + PartialEq> {
    locks: Arc<LocksVec<T>>,
    random_state: RandomState,
}

impl<T: Hash + Eq + PartialEq> LockGuards<T> {
    pub fn new() -> Self {
        Self {
            random_state: RandomState::new(),
            locks: Arc::new(
                (0..LOCK_SHARDING)
                    .map(|_| RwLock::new(HashMap::new()))
                    .collect(),
            ),
        }
    }

    pub async fn get_lock(&self, key: T) -> Arc<Mutex<()>> {
        let hash = self.random_state.hash_one(&key) as usize;
        let lock_shard = hash % self.locks.len();

        let lock = {
            let read = self.locks[lock_shard].read().await;

            read.get(&key).cloned()
        };

        if let Some(lock) = lock {
            lock
        } else {
            let lock = {
                let mut write = self.locks[lock_shard].write().await;

                write
                    .entry(key)
                    .or_insert_with(|| Arc::new(Mutex::new(())))
                    .clone()
            };

            lock
        }
    }
}
