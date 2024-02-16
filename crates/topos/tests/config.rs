use assert_cmd::prelude::*;
use regex::Regex;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::tempdir;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use toml::map::Map;
use toml::Value;
use topos_test_sdk::create_folder;

use crate::utils::setup_polygon_edge;

mod utils;

mod serial_integration {
    use rstest::rstest;
    use sysinfo::{Pid, PidExt, ProcessExt, Signal, System, SystemExt};

    use super::*;

    #[rstest]
    #[tokio::test]
    async fn handle_command_init(
        #[from(create_folder)] home: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = setup_polygon_edge(home.to_str().unwrap()).await;

        let mut cmd = Command::cargo_bin("topos")?;
        cmd.arg("node")
            .arg("--edge-path")
            .arg(path)
            .arg("init")
            .arg("--home")
            .arg(home.to_str().unwrap());

        let output = cmd.assert().success();
        let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

        assert!(result.contains("Created node config file"));

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

    #[tokio::test]
    async fn handle_command_init_without_polygon_edge() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_home_dir = tempdir()?;

        let mut cmd = Command::cargo_bin("topos")?;
        cmd.arg("node")
            .arg("init")
            .arg("--home")
            .arg(tmp_home_dir.path().to_str().unwrap())
            .arg("--no-edge-process");

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
    #[rstest]
    #[tokio::test]
    async fn command_init_precedence_env(
        create_folder: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tmp_home_directory = create_folder;
        // Test node init with env variables
        let node_init_home_env = tmp_home_directory.to_str().unwrap();
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
        assert!(config_contents.contains("name = \"TEST_NODE_ENV\""));
        assert!(config_contents.contains("role = \"fullnode\""));
        assert!(config_contents.contains("subnet = \"topos-env\""));

        Ok(())
    }

    /// Test node cli arguments precedence over env arguments
    #[tokio::test]
    async fn command_init_precedence_cli_env() -> Result<(), Box<dyn std::error::Error>> {
        let tmp_home_dir_env = create_folder("command_init_precedence_cli_env");
        let tmp_home_dir_cli = create_folder("command_init_precedence_cli_env");

        // Test node init with both cli and env flags
        // Cli arguments should take precedence over env variables
        let node_init_home_env = tmp_home_dir_env.to_str().unwrap();
        let node_edge_path_env = setup_polygon_edge(node_init_home_env).await;
        let node_init_name_env = "TEST_NODE_ENV";
        let node_init_role_env = "full-node";
        let node_init_subnet_env = "topos-env";
        let node_init_home_cli = tmp_home_dir_cli.to_str().unwrap();
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
    #[rstest]
    #[test_log::test(tokio::test)]
    async fn command_node_up(
        #[from(create_folder)] tmp_home_dir: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create config file
        let node_up_home_env = tmp_home_dir.to_str().unwrap();
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
        utils::generate_polygon_edge_genesis_file(
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
        let output = tokio::time::timeout(std::time::Duration::from_secs(10), cmd.wait()).await;

        // Check if node up was successful
        match output {
            Ok(Ok(exit_status)) => {
                if !exit_status.success() {
                    println!("Exited with error output {:?}", exit_status.code());
                    cmd.kill().await?;
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

    /// Test node up running from config file
    #[rstest::rstest]
    #[test_log::test(tokio::test)]
    async fn command_node_up_with_old_config(
        #[from(create_folder)] tmp_home_dir: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create config file
        let node_up_home_env = tmp_home_dir.to_str().unwrap();
        let node_edge_path_env = setup_polygon_edge(node_up_home_env).await;
        let node_up_name_env = "test_node_up_old_config";
        let node_up_subnet_env = "topos";

        let mut cmd = Command::cargo_bin("topos")?;
        cmd.arg("node")
            .env("TOPOS_POLYGON_EDGE_BIN_PATH", &node_edge_path_env)
            .env("TOPOS_HOME", node_up_home_env)
            .env("TOPOS_NODE_NAME", node_up_name_env)
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

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(config_path.clone())
            .await?;

        let mut buf = String::new();
        let _ = file.read_to_string(&mut buf).await?;

        let mut current: Map<String, Value> = toml::from_str(&buf)?;
        let tce = current.get_mut("tce").unwrap();

        if let Value::Table(tce_table) = tce {
            tce_table.insert(
                "libp2p-api-addr".to_string(),
                Value::String("0.0.0.0:9091".to_string()),
            );
            tce_table.insert("minimum-tce-cluster-size".to_string(), Value::Integer(0));
            tce_table.insert("network-bootstrap-timeout".to_string(), Value::Integer(5));
            tce_table.remove("p2p");
        } else {
            panic!("TCE configuration table malformed");
        }

        let _ = file.set_len(0).await;
        let _ = file.seek(std::io::SeekFrom::Start(0)).await;
        let _ = file.write_all(toml::to_string(&current)?.as_bytes()).await;

        drop(file);

        // Generate polygon edge genesis file
        let polygon_edge_bin = format!("{}/polygon-edge", node_edge_path_env);
        utils::generate_polygon_edge_genesis_file(
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
            .env("TOPOS_NODE_NAME", node_up_name_env)
            .env("TOPOS_HOME", node_up_home_env)
            .env("RUST_LOG", "topos=info")
            .arg("up")
            .stdout(Stdio::piped());

        let cmd = tokio::process::Command::from(cmd).spawn().unwrap();
        let pid = cmd.id().unwrap();
        let _ = tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        let s = System::new_all();
        if let Some(process) = s.process(Pid::from_u32(pid)) {
            if process.kill_with(Signal::Term).is_none() {
                eprintln!("This signal isn't supported on this platform");
            }
        }

        if let Ok(output) = cmd.wait_with_output().await {
            assert!(output.status.success());
            let stdout = output.stdout;
            let stdout = String::from_utf8_lossy(&stdout);

            let reg =
                Regex::new(r#"Local node is listening on "\/ip4\/.*\/tcp\/9091\/p2p\/"#).unwrap();
            assert!(reg.is_match(&stdout));
        } else {
            panic!("Failed to shutdown gracefully");
        }
        // Cleanup
        std::fs::remove_dir_all(node_up_home_env)?;

        Ok(())
    }
}
