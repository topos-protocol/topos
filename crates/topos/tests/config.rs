use assert_cmd::prelude::*;
use rstest::{fixture, rstest};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tar::Archive;

use tracing::info;

const POLYGON_EDGE_URI: &str = " https://github.com/topos-protocol/polygon-edge/releases/download/v0.8.0-develop-20230503/polygon-edge_0.8.0-develop-20230503_darwin_arm64.tar.gz";
const POLYGON_EDGE_ARCHIVE_NAME: &str = "polygon-edge_0.8.0-develop-20230503_darwin_arm64.tar.gz";
const BINARY_FOLDER: &str = "./tests/test-binary";

#[fixture]
fn polygon_edge_path() -> String {
    let current_dir = std::env::current_dir().expect("Failed to get current dir");
    let binary_path = current_dir.join(BINARY_FOLDER).join("polygon-edge");

    if PathBuf::from(binary_path).exists() {
        info!("polygob-edge binary already downloaded");
        return BINARY_FOLDER.to_string();
    } else {
        info!("Downloading polygon-edge binary");
        std::fs::create_dir_all(&current_dir.join(BINARY_FOLDER))
            .expect("Failed to create binary folder");
    }

    let response =
        reqwest::blocking::get(POLYGON_EDGE_URI).expect("Failed to download polygon edge binary");

    let archive_download_path = current_dir
        .join(BINARY_FOLDER)
        .join(POLYGON_EDGE_ARCHIVE_NAME);

    // Creating the archive file
    let mut archive_file =
        std::fs::File::create(&archive_download_path).expect("Failed to create archive file");

    // Filling it with the downloaded data
    archive_file
        .write_all(response.bytes().expect("Failed to read response").as_ref())
        .expect("Failed to write archive file");

    // Decompressing the archive
    let archive_file =
        std::fs::File::open(&archive_download_path).expect("Failed to open archive file");
    let mut archive = Archive::new(flate2::read::GzDecoder::new(archive_file));
    archive
        .unpack(current_dir.join(BINARY_FOLDER))
        .expect("Cannot unpack archive");

    // Removing the archive file
    std::fs::remove_file(&archive_download_path).expect("Failed to remove archive file");

    // Returning the path to the binary
    BINARY_FOLDER.to_string()
}

#[rstest]
#[test]
fn test_handle_command_init(polygon_edge_path: String) -> Result<(), Box<dyn std::error::Error>> {
    let temporary_test_folder = "/tmp/topos/handle_command_init";

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("--edge-path")
        .arg(polygon_edge_path)
        .arg("init")
        .arg("--home")
        .arg(temporary_test_folder);

    let output = cmd.assert().success();
    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Created node config file"));

    let home = PathBuf::from(temporary_test_folder);

    // Verification: check that the config file was created
    let config_path = home.join("node").join("default").join("config.toml");
    assert!(config_path.exists());

    // Further verification might include checking the contents of the config file
    let config_contents = std::fs::read_to_string(&config_path).unwrap();

    assert!(config_contents.contains("[base]"));
    assert!(config_contents.contains("name = \"default\""));
    assert!(config_contents.contains("[subnet]"));
    assert!(config_contents.contains("[tce]"));

    std::fs::remove_dir_all(temporary_test_folder)?;

    Ok(())
}

#[test]
fn test_nothing_written_if_failure() -> Result<(), Box<dyn std::error::Error>> {
    let temporary_test_folder = "/tmp/topos/test_nothing_written_if_failure";

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("--edge-path")
        .arg("./inexistent/folder/") // Command will fail
        .arg("init")
        .arg("--home")
        .arg(temporary_test_folder);

    // Should fail
    cmd.assert().failure();

    let home = PathBuf::from(temporary_test_folder);

    // Check that files were NOT created
    let config_path = home.join("node").join("default");
    assert!(!config_path.exists());

    std::fs::remove_dir_all(temporary_test_folder)?;

    Ok(())
}

#[rstest]
#[test]
fn test_handle_command_init_with_custom_name(
    polygon_edge_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let temporary_test_folder = "/tmp/topos/test_handle_command_init_with_custom_name";
    let node_name = "TEST_NODE";

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("--edge-path")
        .arg(polygon_edge_path)
        .arg("init")
        .arg("--home")
        .arg(temporary_test_folder)
        .arg("--name")
        .arg(node_name);

    let output = cmd.assert().success();
    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Created node config file"));

    let home = PathBuf::from(temporary_test_folder);

    // Verification: check that the config file was created
    let config_path = home.join("node").join(node_name).join("config.toml");
    assert!(config_path.exists());

    // Further verification might include checking the contents of the config file
    let config_contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(config_contents.contains("[base]"));
    assert!(config_contents.contains(node_name));
    assert!(config_contents.contains("[tce]"));

    std::fs::remove_dir_all(temporary_test_folder)?;

    Ok(())
}
