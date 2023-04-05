use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn network_spam_help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("network").arg("spam").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(result);

    Ok(())
}
