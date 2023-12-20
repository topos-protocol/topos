mod utils;

use std::{
    path::PathBuf,
    process::{Command, ExitStatus},
    thread,
};

use assert_cmd::prelude::*;
use sysinfo::{Pid, PidExt, ProcessExt, Signal, System, SystemExt};
use tempfile::tempdir;

use crate::utils::generate_polygon_edge_genesis_file;

#[test]
fn help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(utils::sanitize_config_folder_path(result));

    Ok(())
}

/// Test node up running from config file
#[test_log::test(tokio::test)]
async fn command_node_up_sigterm() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;

    // Create config file
    let node_up_home_env = tmp_home_dir.path().to_str().unwrap();
    let node_edge_path_env = utils::setup_polygon_edge(node_up_home_env).await;
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
    let pid = cmd.id().unwrap();
    let sigterm_wait = tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let s = System::new_all();
    if let Some(process) = s.process(Pid::from_u32(pid)) {
        if process.kill_with(Signal::Term).is_none() {
            eprintln!("This signal isn't supported on this platform");
        }
    }

    if let Ok(code) = cmd.wait().await {
        assert!(code.success());
    } else {
        panic!("Failed to gracefull shutdown");
    }

    // Cleanup
    std::fs::remove_dir_all(node_up_home_env)?;

    Ok(())
}
