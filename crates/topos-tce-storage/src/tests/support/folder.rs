use std::{
    fs,
    path::{Path, PathBuf},
};

use rand::Rng;
use rstest::fixture;

#[fixture]
pub(crate) fn random_path() -> Box<PathBuf> {
    let mut rng = rand::thread_rng();

    let random: u32 = rng.gen();
    Box::new(Path::new(&format!("./tests/databases/{random}")).to_path_buf())
}

pub(crate) fn created_folder(random_path: &PathBuf) {
    fs::create_dir_all(random_path.clone()).unwrap();
}
