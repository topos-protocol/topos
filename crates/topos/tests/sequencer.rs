mod utils;

use std::process::Command;

use assert_cmd::prelude::*;

#[test]
fn sequencer_help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("sequencer").arg("run").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(utils::sanitize_config_folder_path(result));

    Ok(())
}
