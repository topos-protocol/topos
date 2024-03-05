use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
};

use once_cell::sync::Lazy;
use rocksdb::Options;
use rstest::fixture;
use topos_test_sdk::storage::create_folder;

use crate::{
    epoch::{EpochValidatorsStore, ValidatorPerEpochStore},
    fullnode::FullNodeStore,
    index::IndexTables,
    rocks::{db::init_db, db::RocksDB},
    validator::{ValidatorPerpetualTables, ValidatorStore},
};

use self::folder::created_folder;

pub(crate) mod columns;
pub(crate) mod folder;

pub(crate) static DB: Lazy<Mutex<HashMap<&'static str, Arc<RocksDB>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[fixture]
pub(crate) fn database_name() -> &'static str {
    Box::leak(Box::new(
        topos_test_sdk::storage::create_folder(thread::current().name().unwrap())
            .to_str()
            .unwrap()
            .replace("::", "_"),
    ))
}

#[fixture]
pub(crate) fn store() -> Arc<ValidatorStore> {
    let temp_dir = create_folder::default();
    let perpetual_tables = Arc::new(ValidatorPerpetualTables::open(&temp_dir));
    let index_tables = Arc::new(IndexTables::open(&temp_dir));

    let participants_store =
        EpochValidatorsStore::new(&temp_dir).expect("Unable to create Participant store");

    let epoch_store =
        ValidatorPerEpochStore::new(0, &temp_dir).expect("Unable to create Per epoch store");

    let store = FullNodeStore::open(
        epoch_store,
        participants_store,
        perpetual_tables,
        index_tables,
    )
    .expect("Unable to create full node store");

    ValidatorStore::open(&temp_dir, store).unwrap()
}

#[fixture]
pub(crate) fn rocks_db(database_name: &'static str) -> Arc<RocksDB> {
    let mut dbs = DB.lock().unwrap();

    dbs.entry(database_name)
        .or_insert_with(|| {
            let path = PathBuf::from_str(database_name).unwrap();
            created_folder(&path);
            let mut options = Options::default();
            options.create_if_missing(true);
            options.create_missing_column_families(true);

            Arc::new(init_db(&path, options).unwrap())
        })
        .clone()
}
