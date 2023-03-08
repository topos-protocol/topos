use std::{
    path::PathBuf,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use futures::Stream;
use topos_core::uci::Certificate;
use topos_tce_storage::{
    events::StorageEvent, Connection, ConnectionBuilder, RocksDBStorage, Storage, StorageClient,
};

pub async fn create_rocksdb<'a, C: IntoIterator<Item = &'a Certificate>>(
    folder_name: &str,
    certificates: C,
) -> (
    PathBuf,
    (
        ConnectionBuilder<RocksDBStorage>,
        StorageClient,
        impl Stream<Item = StorageEvent>,
    ),
) {
    let dir = env!("TOPOS_TEST_SDK_TMP");
    let mut temp_dir =
        std::path::PathBuf::from_str(dir).expect("Unable to read CARGO_TARGET_TMPDIR");
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    temp_dir.push(format!(
        "./{}/data_{}/rocksdb",
        folder_name,
        time.as_nanos()
    ));

    let storage = RocksDBStorage::with_isolation(&temp_dir).expect("valid rocksdb storage");
    for certificate in certificates {
        _ = storage.persist(certificate, None).await;
    }
    (temp_dir, Connection::build(Box::pin(async { Ok(storage) })))
}
