use futures::{Future, Stream};
use rstest::fixture;
use std::future::IntoFuture;
use std::{
    path::PathBuf,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::spawn;

use topos_core::uci::Certificate;
use topos_tce_storage::{
    events::StorageEvent, Connection, ConnectionBuilder, RocksDBStorage, Storage, StorageClient,
};

#[fixture]
fn folder_name<'a>() -> &'a str {
    "test"
}

#[fixture(certificates = Vec::new())]
pub async fn storage_client(certificates: Vec<Certificate>) -> StorageClient {
    spawn_runtime(create_rocksdb("test", certificates)).await
}

async fn spawn_runtime(
    create_rocksdb: impl Future<
        Output = (
            PathBuf,
            (
                ConnectionBuilder<RocksDBStorage>,
                StorageClient,
                impl Stream<Item = StorageEvent>,
            ),
        ),
    >,
) -> StorageClient {
    let (_, (runtime, client, _stream)) = create_rocksdb.await;

    spawn(runtime.into_future());

    client
}

pub fn create_folder(folder_name: &str) -> PathBuf {
    let dir = env!("TOPOS_TEST_SDK_TMP");
    let mut temp_dir =
        std::path::PathBuf::from_str(dir).expect("Unable to read CARGO_TARGET_TMPDIR");
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    temp_dir.push(format!(
        "./{}/data_{}/rocksdb",
        folder_name,
        time.as_nanos()
    ));

    temp_dir
}

#[fixture(certificates = Vec::new())]
pub async fn create_rocksdb(
    folder_name: &str,
    certificates: Vec<Certificate>,
) -> (
    PathBuf,
    (
        ConnectionBuilder<RocksDBStorage>,
        StorageClient,
        impl Stream<Item = StorageEvent>,
    ),
) {
    let temp_dir = create_folder(folder_name);

    let storage = RocksDBStorage::with_isolation(&temp_dir).expect("valid rocksdb storage");
    for certificate in certificates {
        _ = storage.persist(&certificate, None).await;
    }
    (temp_dir, Connection::build(Box::pin(async { Ok(storage) })))
}
