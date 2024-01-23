use assert_cmd::prelude::*;
use predicates::prelude::*;
use regex::Regex;
use std::path::PathBuf;
use std::process::Command;
use topos::install_polygon_edge;

// Have to allow dead_code because clippy doesn't recognize it is being used in the tests
#[cfg(test)]
#[allow(dead_code)]
pub fn sanitize_config_folder_path(cmd_out: &str) -> String {
    // Sanitize the result here:
    // When run locally, we get /Users/<username>/.config/topos
    // When testing on the CI, we get /home/runner/.config/topos
    let pattern = Regex::new(r"\[default: .+?/.config/topos\]").unwrap();
    pattern
        .replace(cmd_out, "[default: /home/runner/.config/topos]")
        .to_string()
}

// Have to allow dead_code because clippy doesn't recognize it is being used in the tests
#[allow(dead_code)]
pub async fn setup_polygon_edge(path: &str) -> String {
    let installation_path = std::env::current_dir().unwrap().join(path);
    let binary_path = installation_path.join("polygon-edge");

    if !binary_path.exists() {
        std::fs::create_dir_all(installation_path.clone())
            .expect("Cannot create test binary folder");

        install_polygon_edge(
            "topos-protocol/polygon-edge".to_string(),
            None,
            installation_path.clone().as_path(),
        )
        .await
        .expect("Cannot install Polygon Edge binary");
    }

    installation_path.to_str().unwrap().to_string()
}

// Have to allow dead_code because clippy doesn't recognize it is being used in the tests
#[allow(dead_code)]
pub async fn generate_polygon_edge_genesis_file(
    polygon_edge_bin: &str,
    home_path: &str,
    node_name: &str,
    subnet: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let genesis_folder_path: PathBuf = PathBuf::from(format!("{}/subnet/{}", home_path, subnet));
    if !genesis_folder_path.exists() {
        std::fs::create_dir_all(genesis_folder_path.clone())
            .expect("Cannot create subnet genesis file folder");
    }
    let genesis_file_path = format!("{}/genesis.json", genesis_folder_path.display());
    println!("Polygon edge path: {}", polygon_edge_bin);
    let mut cmd = Command::new(polygon_edge_bin);
    let val_prefix_path = format!("{}/node/{}/", home_path, node_name);
    cmd.arg("genesis")
        .arg("--dir")
        .arg(&genesis_file_path)
        .arg("--consensus")
        .arg("ibft")
        .arg("--ibft-validators-prefix-path")
        .arg(val_prefix_path)
        .arg("--bootnode") /* set dummy bootnode, we will not run edge to produce blocks */
        .arg("/ip4/127.0.0.1/tcp/8545/p2p/16Uiu2HAmNYneHCbJ1Ntz1ojvTdiNGCMGWNT5MGMH28AzKNV66Paa");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Genesis written to {}",
            genesis_folder_path.display()
        )));
    Ok(())
}
