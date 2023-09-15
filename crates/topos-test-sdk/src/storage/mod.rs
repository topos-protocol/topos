use futures::{Future, Stream};
use rand::Rng;
use rstest::fixture;
use std::future::IntoFuture;
use std::sync::Arc;
use std::{
    path::PathBuf,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::spawn;

use topos_tce_storage::{
    epoch::EpochValidatorsStore, epoch::ValidatorPerEpochStore, events::StorageEvent,
    fullnode::FullNodeStore, index::IndexTables, store::WriteStore, types::CertificateDelivered,
    validator::ValidatorPerpetualTables, validator::ValidatorStore, Connection, ConnectionBuilder,
    RocksDBStorage, Storage, StorageClient,
};

#[fixture]
fn folder_name<'a>() -> &'a str {
    "test"
}

#[fixture(certificates = Vec::new())]
pub async fn storage_client(certificates: Vec<CertificateDelivered>) -> StorageClient {
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
    let mut rng = rand::thread_rng();

    temp_dir.push(format!(
        "./{}/data_{}_{}/rocksdb",
        folder_name,
        time.as_nanos(),
        rng.gen::<u64>()
    ));

    temp_dir
}

#[fixture(certificates = Vec::new())]
pub async fn create_validator_store(
    folder_name: &str,
    certificates: Vec<CertificateDelivered>,
) -> (PathBuf, Arc<ValidatorStore>) {
    let (temp_dir, full_node_store) = create_fullnode_store(folder_name, certificates).await;

    let store = ValidatorStore::open(temp_dir.clone(), full_node_store)
        .expect("Unable to create validator store");

    (temp_dir, store)
}

#[fixture(certificates = Vec::new())]
pub async fn create_fullnode_store(
    folder_name: &str,
    certificates: Vec<CertificateDelivered>,
) -> (PathBuf, Arc<FullNodeStore>) {
    let temp_dir = create_folder(folder_name);

    let perpetual_tables = Arc::new(ValidatorPerpetualTables::open(temp_dir.clone()));
    let index_tables = Arc::new(IndexTables::open(temp_dir.clone()));

    let validators_store = EpochValidatorsStore::new(temp_dir.clone())
        .expect("Unable to create EpochValidators store");

    let epoch_store =
        ValidatorPerEpochStore::new(0, temp_dir.clone()).expect("Unable to create Per epoch store");

    let store = FullNodeStore::open(
        epoch_store,
        validators_store,
        perpetual_tables,
        index_tables,
    )
    .expect("Unable to create full node store");

    store
        .multi_insert_certificates_delivered(&certificates[..])
        .await
        .unwrap();

    (temp_dir, store)
}

#[fixture(certificates = Vec::new())]
pub async fn create_rocksdb(
    folder_name: &str,
    certificates: Vec<CertificateDelivered>,
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
        _ = storage.persist(&certificate.certificate, None).await;
    }

    (temp_dir, Connection::build(Box::pin(async { Ok(storage) })))
}
