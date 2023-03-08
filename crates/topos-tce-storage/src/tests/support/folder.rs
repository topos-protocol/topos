use std::{fs, path::PathBuf, thread};

use rstest::fixture;

#[fixture]
pub(crate) fn random_path() -> Box<PathBuf> {
    let temp_dir = topos_test_sdk::storage::create_folder(thread::current().name().unwrap());
    Box::new(temp_dir)
}

pub(crate) fn created_folder(random_path: &PathBuf) {
    fs::create_dir_all(random_path.clone()).unwrap();
}
