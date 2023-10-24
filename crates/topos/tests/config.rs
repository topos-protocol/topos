use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;
use topos::install_polygon_edge;

async fn setup_polygon_edge(path: &str) -> String {
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

async fn generate_polygon_edge_genesis_file(
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

#[tokio::test]
async fn handle_command_init() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;
    let path = setup_polygon_edge(tmp_home_dir.path().to_str().unwrap()).await;

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("--edge-path")
        .arg(path)
        .arg("init")
        .arg("--home")
        .arg(tmp_home_dir.path().to_str().unwrap());

    let output = cmd.assert().success();
    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Created node config file"));

    let home = PathBuf::from(tmp_home_dir.path());

    // Verification: check that the config file was created
    let config_path = home.join("node").join("default").join("config.toml");
    assert!(config_path.exists());

    // Further verification might include checking the contents of the config file
    let config_contents = std::fs::read_to_string(&config_path).unwrap();

    assert!(config_contents.contains("[base]"));
    assert!(config_contents.contains("name = \"default\""));
    assert!(config_contents.contains("[edge]"));
    assert!(config_contents.contains("[tce]"));

    Ok(())
}

#[test]
fn nothing_written_if_failure() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("--edge-path")
        .arg("./inexistent/folder/") // Command will fail
        .arg("init")
        .arg("--home")
        .arg(tmp_home_dir.path().to_str().unwrap());

    // Should fail
    cmd.assert().failure();

    let home = PathBuf::from(tmp_home_dir.path().to_str().unwrap());

    // Check that files were NOT created
    let config_path = home.join("node").join("default");
    assert!(!config_path.exists());

    Ok(())
}

#[rstest]
#[tokio::test]
async fn handle_command_init_with_custom_name() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;
    let node_name = "TEST_NODE";
    let path = setup_polygon_edge(tmp_home_dir.path().to_str().unwrap()).await;

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("--edge-path")
        .arg(path.clone())
        .arg("init")
        .arg("--home")
        .arg(tmp_home_dir.path().to_str().unwrap())
        .arg("--name")
        .arg(node_name);

    let output = cmd.assert().success();
    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Created node config file"));

    let home = PathBuf::from(path);

    // Verification: check that the config file was created
    let config_path = home.join("node").join(node_name).join("config.toml");
    assert!(config_path.exists());

    // Further verification might include checking the contents of the config file
    let config_contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(config_contents.contains("[base]"));
    assert!(config_contents.contains(node_name));
    assert!(config_contents.contains("[tce]"));

    Ok(())
}

/// Test node init env arguments
#[tokio::test]
async fn command_init_precedence_env() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_directory = tempdir()?;

    // Test node init with env variables
    let node_init_home_env = tmp_home_directory.path().to_str().unwrap();
    let node_edge_path_env = setup_polygon_edge(node_init_home_env).await;
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
    let node_edge_path_env = setup_polygon_edge(node_init_home_env).await;
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

/// Test node up running from config file
#[test_log::test(tokio::test)]
async fn command_node_up() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;

    // Create config file
    let node_up_home_env = tmp_home_dir.path().to_str().unwrap();
    let node_edge_path_env = setup_polygon_edge(node_up_home_env).await;
    let node_up_name_env = "TEST_NODE_UP";
    let node_up_role_env = "full-node";
    let node_up_subnet_env = "topos-up-env-subnet";

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .env("TOPOS_POLYGON_EDGE_BIN_PATH", &node_edge_path_env)
        .env("TOPOS_HOME", node_up_home_env)
        .env("TOPOS_NODE_NAME", node_up_name_env)
        .env("TOPOS_NODE_ROLE", node_up_role_env)
        .env("TOPOS_NODE_SUBNET", node_up_subnet_env)
        .arg("init");

    let output = cmd.assert().success();
    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;
    assert!(result.contains("Created node config file"));

    // Run node init with cli flags
    let home = PathBuf::from(node_up_home_env);
    // Verification: check that the config file was created
    let config_path = home.join("node").join(node_up_name_env).join("config.toml");
    assert!(config_path.exists());

    // Generate polygon edge genesis file
    let polygon_edge_bin = format!("{}/polygon-edge", node_edge_path_env);
    generate_polygon_edge_genesis_file(
        &polygon_edge_bin,
        node_up_home_env,
        node_up_name_env,
        node_up_subnet_env,
    )
    .await?;
    let polygon_edge_genesis_path = home
        .join("subnet")
        .join(node_up_subnet_env)
        .join("genesis.json");
    assert!(polygon_edge_genesis_path.exists());

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .env("TOPOS_POLYGON_EDGE_BIN_PATH", &node_edge_path_env)
        .env("TOPOS_HOME", node_up_home_env)
        .env("TOPOS_NODE_NAME", node_up_name_env)
        .arg("up");
    let mut cmd = tokio::process::Command::from(cmd).spawn().unwrap();
    let output = tokio::time::timeout(std::time::Duration::from_secs(60), cmd.wait()).await;

    // Check if node up was successful
    match output {
        Ok(Ok(exit_status)) => {
            if !exit_status.success() {
                println!("Exited with error output {:?}", exit_status.code());
                panic!("Node up failed");
            }
        }
        Ok(Err(e)) => {
            println!("Node exited with error: {e}");
            // Kill the subprocess
            cmd.kill().await?;
            panic!("Node up failed");
        }
        Err(_) => {
            println!("Node up is running correctly, time-outed");
            // Kill the subprocess
            cmd.kill().await?;
        }
    }

    // Cleanup
    std::fs::remove_dir_all(node_up_home_env)?;

    Ok(())
}
