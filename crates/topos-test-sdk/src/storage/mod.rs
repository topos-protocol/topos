use rstest::fixture;
use std::path::PathBuf;
use std::sync::Arc;

use topos_core::types::CertificateDelivered;
use topos_tce_storage::{
    epoch::EpochValidatorsStore, epoch::ValidatorPerEpochStore, fullnode::FullNodeStore,
    index::IndexTables, store::WriteStore, validator::ValidatorPerpetualTables,
    validator::ValidatorStore, StorageClient,
};

use crate::folder_name;

#[fixture(certificates = Vec::new())]
pub async fn storage_client(certificates: Vec<CertificateDelivered>) -> StorageClient {
    let store = create_validator_store::partial_1(certificates).await;

    StorageClient::new(store)
}

#[fixture]
pub fn create_folder(folder_name: &str) -> PathBuf {
    let mut path = crate::create_folder(folder_name);

    path.push("rocksdb");

    path
}

#[fixture(certificates = Vec::new())]
pub async fn create_validator_store(
    certificates: Vec<CertificateDelivered>,
    #[future] create_fullnode_store: Arc<FullNodeStore>,
) -> Arc<ValidatorStore> {
    let temp_dir = create_folder::default();
    let fullnode_store = create_fullnode_store.await;

    let store =
        ValidatorStore::open(temp_dir, fullnode_store).expect("Unable to create validator store");

    store
        .insert_certificates_delivered(&certificates)
        .await
        .expect("Unable to insert predefined certificates");

    store
}

pub async fn create_validator_store_with_fullnode(
    fullnode_store: Arc<FullNodeStore>,
) -> Arc<ValidatorStore> {
    ValidatorStore::open(create_folder::default(), fullnode_store)
        .expect("Unable to create validator store")
}
#[fixture(certificates = Vec::new())]
pub async fn create_fullnode_store(certificates: Vec<CertificateDelivered>) -> Arc<FullNodeStore> {
    let temp_dir = create_folder::default();

    let perpetual_tables = Arc::new(ValidatorPerpetualTables::open(temp_dir.clone()));
    let index_tables = Arc::new(IndexTables::open(temp_dir.clone()));

    let validators_store = EpochValidatorsStore::new(temp_dir.clone())
        .expect("Unable to create EpochValidators store");

    let epoch_store =
        ValidatorPerEpochStore::new(0, temp_dir).expect("Unable to create Per epoch store");

    let store = FullNodeStore::open(
        epoch_store,
        validators_store,
        perpetual_tables,
        index_tables,
    )
    .expect("Unable to create full node store");

    store
        .insert_certificates_delivered(&certificates[..])
        .await
        .unwrap();

    store
}
