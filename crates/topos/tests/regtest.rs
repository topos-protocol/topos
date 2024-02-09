mod utils;

use std::process::Command;

use assert_cmd::prelude::*;

#[test]
fn regtest_spam_help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("regtest").arg("spam").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(utils::sanitize_config_folder_path(result));

    Ok(())
}

#[test]
fn regtest_spam_invalid_hosts() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("regtest")
        .arg("spam")
        .arg("--benchmark")
        .arg("--target-hosts")
        .arg("asd")
        .arg("--number")
        .arg("1");

    let output = cmd.assert().failure();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains(
        "Invalid target-hosts pattern. Has to be in the format of http://validator-1:9090"
    ));

    Ok(())
}

#[test]
fn regtest_spam_invalid_number() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("regtest")
        .arg("spam")
        .arg("--benchmark")
        .arg("--target-hosts")
        .arg(" http://validator-{N}:9090")
        .arg("--number")
        .arg("dasd");

    cmd.assert().failure();

    Ok(())
}
