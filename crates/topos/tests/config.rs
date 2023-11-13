use assert_cmd::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;
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
async fn handle_command_init() -> Result<(), Box<dyn std::error::Error>> {
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
    assert!(config_contents.contains("[edge]"));
    assert!(config_contents.contains("[tce]"));

    std::fs::remove_dir_all(temporary_test_folder)?;

    Ok(())
}

#[tokio::test]
async fn handle_command_up() -> Result<(), Box<dyn std::error::Error>> {
    let temporary_test_folder = "/tmp/topos/handle_command_up";
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

    std::fs::remove_dir_all(temporary_test_folder)?;

    Ok(())
}

#[test]
fn nothing_written_if_failure() -> Result<(), Box<dyn std::error::Error>> {
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
async fn handle_command_init_with_custom_name() -> Result<(), Box<dyn std::error::Error>> {
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

/// Test node init env arguments
#[tokio::test]
async fn command_init_precedence_env() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_directory = tempdir()?;

    // Test node init with env variables
    let node_init_home_env = tmp_home_directory.path().to_str().unwrap();
    let node_edge_path_env = polygon_edge_path(node_init_home_env).await;
    let node_init_name_env = "TEST_NODE_ENV";
    let node_init_role_env = "full-node";
    let node_init_subnet_env = "topos-env";

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .env("TOPOS_POLYGON_EDGE_BIN_PATH", &node_edge_path_env)
        .env("TOPOS_HOME", node_init_home_env)
        .env("TOPOS_NODE_NAME", node_init_name_env)
        .env("TOPOS_NODE_ROLE", node_init_role_env)
        .env("TOPOS_NODE_SUBNET", node_init_subnet_env)
        .arg("init");

    let output = cmd.assert().success();
    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    // Test node init with cli flags
    assert!(result.contains("Created node config file"));
    println!("YES");
    let home = PathBuf::from(node_init_home_env);
    // Verification: check that the config file was created
    let config_path = home
        .join("node")
        .join(node_init_name_env)
        .join("config.toml");
    assert!(config_path.exists());
    // Check if config file params are according to env params
    let config_contents = std::fs::read_to_string(&config_path).unwrap();
    println!("{:#?}", config_contents);
    assert!(config_contents.contains("name = \"TEST_NODE_ENV\""));
    assert!(config_contents.contains("role = \"fullnode\""));
    assert!(config_contents.contains("subnet = \"topos-env\""));

    Ok(())
}

/// Test node cli arguments precedence over env arguments
#[tokio::test]
async fn command_init_precedence_cli_env() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;

    // Test node init with both cli and env flags
    // Cli arguments should take precedence over env variables
    let node_init_home_env = tmp_home_dir.path().to_str().unwrap();
    let node_edge_path_env = polygon_edge_path(node_init_home_env).await;
    let node_init_name_env = "TEST_NODE_ENV";
    let node_init_role_env = "full-node";
    let node_init_subnet_env = "topos-env";
    let node_init_home_cli = "/tmp/topos/test_command_init_precedence_cli";
    let node_edge_path_cli = node_edge_path_env.clone();
    let node_init_name_cli = "TEST_NODE_CLI";
    let node_init_role_cli = "sequencer";
    let node_init_subnet_cli = "topos-cli";

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .env("TOPOS_POLYGON_EDGE_BIN_PATH", &node_edge_path_env)
        .env("TOPOS_HOME", node_init_home_env)
        .env("TOPOS_NODE_NAME", node_init_name_env)
        .env("TOPOS_NODE_ROLE", node_init_role_env)
        .env("TOPOS_NODE_SUBNET", node_init_subnet_env)
        .arg("--edge-path")
        .arg(node_edge_path_cli)
        .arg("init")
        .arg("--name")
        .arg(node_init_name_cli)
        .arg("--home")
        .arg(node_init_home_cli)
        .arg("--role")
        .arg(node_init_role_cli)
        .arg("--subnet")
        .arg(node_init_subnet_cli);

    let output = cmd.assert().success();
    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Created node config file"));
    let home = PathBuf::from(node_init_home_cli);
    // Verification: check that the config file was created
    let config_path = home
        .join("node")
        .join(node_init_name_cli)
        .join("config.toml");
    assert!(config_path.exists());
    // Check if config file params are according to cli params
    let config_contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(config_contents.contains("name = \"TEST_NODE_CLI\""));
    assert!(config_contents.contains("role = \"sequencer\""));
    assert!(config_contents.contains("subnet = \"topos-cli\""));

    Ok(())
}
