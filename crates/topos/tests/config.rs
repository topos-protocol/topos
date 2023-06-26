use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;

#[test]
fn test_handle_command_init() -> Result<(), Box<dyn std::error::Error>> {
    let temporary_test_folder = "/tmp/topos";

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("node")
        .arg("init")
        .arg("--home")
        .arg(temporary_test_folder);

    let output = cmd.assert().success();
    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Created node config file"));

    let mut home = PathBuf::from("/tmp");
    home.push("topos");

    // Verification: check that the config file was created
    let config_path = home.join("node").join("default").join("config.toml");
    assert!(config_path.exists());

    // Further verification might include checking the contents of the config file
    let config_contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(config_contents.contains("[base]"));
    assert!(config_contents.contains("[tce]"));
    assert!(config_contents.contains("[sequencer]"));

    std::fs::remove_dir_all("/tmp/topos")?;
    Ok(())
}
