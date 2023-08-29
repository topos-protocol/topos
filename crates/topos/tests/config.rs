use assert_cmd::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use topos::install_polygon_edge;

async fn polygon_edge_path(path: &str) -> String {
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

#[tokio::test]
async fn test_handle_command_init() -> Result<(), Box<dyn std::error::Error>> {
    let temporary_test_folder = "/tmp/topos/handle_command_init";
    let path = polygon_edge_path(temporary_test_folder).await;

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("--edge-path")
        .arg(path)
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

#[tokio::test]
async fn test_handle_command_init_with_custom_name() -> Result<(), Box<dyn std::error::Error>> {
    let temporary_test_folder = "/tmp/topos/test_handle_command_init_with_custom_name";
    let node_name = "TEST_NODE";
    let path = polygon_edge_path(temporary_test_folder).await;

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("--edge-path")
        .arg(path)
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
