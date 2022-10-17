use std::sync::atomic::Ordering;

use rstest::rstest;
use tokio::spawn;

use crate::rocks::db::RocksDB;
use crate::tests::support::rocks_db;

#[rstest]
#[tokio::test]
async fn create_batch_multithread(rocks_db: RocksDB) {
    let mut batch = rocks_db.atomic_batch.lock().await;
    rocks_db.batch_in_progress.store(true, Ordering::SeqCst);

    let rocks_db_clone = rocks_db.clone();
    let join = spawn(async move {
        let rocks_db = rocks_db_clone;
        let mut batch = rocks_db.atomic_batch.lock().await;
        batch.put("key1", "thread_2_value");
        batch.put("key2", "thread_2_value");

        let batch = core::mem::take(&mut *batch);
        _ = rocks_db.rocksdb.write(batch);
    });

    batch.put("key1", "thread_1_value");
    batch.put("key2", "thread_1_value");

    let write_batch = core::mem::take(&mut *batch);
    _ = rocks_db.rocksdb.write(write_batch);

    assert_eq!(
        String::from_utf8(rocks_db.rocksdb.get("key1").unwrap().unwrap()).unwrap(),
        "thread_1_value"
    );

    drop(batch);
    _ = join.await;

    assert_eq!(
        String::from_utf8(rocks_db.rocksdb.get("key1").unwrap().unwrap()).unwrap(),
        "thread_2_value"
    );
}
