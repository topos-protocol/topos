use std::{
    fmt::Debug,
    sync::{atomic::AtomicBool, Arc},
};

use once_cell::sync::OnceCell;
use rocksdb::MultiThreaded;
use tokio::sync::Mutex;

pub(crate) static DB: OnceCell<RocksDB> = OnceCell::new();

#[derive(Clone)]
pub struct RocksDB {
    pub(crate) rocksdb: Arc<rocksdb::DBWithThreadMode<MultiThreaded>>,
    pub(crate) batch_in_progress: Arc<AtomicBool>,
    #[allow(dead_code)]
    pub(crate) atomic_batch: Arc<Mutex<rocksdb::WriteBatch>>,
}

impl Debug for RocksDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RocksDB")
            .field("rocksdb", &self.rocksdb)
            .field("batch_in_progress", &self.batch_in_progress)
            .finish()
    }
}
