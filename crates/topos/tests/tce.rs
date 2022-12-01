use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("tce").arg("run").arg("-h");

    let failure = cmd.assert().success();

    let result: &str = std::str::from_utf8(&failure.get_output().stdout)?;

    insta::assert_snapshot!(result);

    Ok(())
}
