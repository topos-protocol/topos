use std::{env, path::PathBuf, str::FromStr};

fn main() {
    let mut path = PathBuf::from_str(
        &env::var("CARGO_MANIFEST_DIR").expect("unable to build du to missing CARGO_MANIFEST_DIR"),
    )
    .expect("Unable to build PathBuf for topos-test-sdk");

    path.push("./../../target/tmp/");
    let path = path.as_path();
    println!(
        "cargo:rustc-env=TOPOS_TEST_SDK_TMP={}",
        path.to_str().unwrap()
    );
}
