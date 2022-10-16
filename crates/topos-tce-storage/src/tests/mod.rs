use rstest::rstest;
use topos_core::uci::Certificate;

use crate::{RocksDBStorage, Storage};

use self::support::storage;

mod db_columns;
mod rocksdb;
pub(crate) mod support;

#[rstest]
#[tokio::test]
async fn can_persist_a_pending_certificate(mut storage: RocksDBStorage) {
    let certificate = Certificate::new("cert_id".into(), "source_subnet_id".into(), Vec::new());

    assert!(storage.add_pending_certificate(certificate).await.is_ok());
}
