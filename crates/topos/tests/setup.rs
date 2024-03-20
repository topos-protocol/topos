mod utils;

use std::{fs, process::Command};

use assert_cmd::prelude::*;
use tempfile::tempdir;

#[test]
fn setup_subnet_install_edge() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("setup")
        .arg("subnet")
        .arg("--path")
        .arg(tmp_home_dir.path());

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Polygon Edge installation successful"));

    Ok(())
}

#[test]
fn setup_with_no_arguments() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("setup");

    let output = cmd.assert().failure();

    let result: &str = std::str::from_utf8(&output.get_output().stderr)?;

    assert!(result
        .contains("No subcommand provided. You can use `--help` to see available subcommands."));

    Ok(())
}

#[test]
fn setup_subnet_fail_to_install_release() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("setup")
        .arg("subnet")
        .arg("--path")
        .arg(tmp_home_dir.path())
        .arg("--release")
        .arg("invalid");

    let output = cmd.assert().failure();

    let result: &str = std::str::from_utf8(&output.get_output().stderr)?;

    assert!(result.contains(
        "Error installing Polygon Edge: There is no valid Polygon Edge release available"
    ));

    Ok(())
}

#[test]
fn setup_subnet_install_edge_custom_path() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;
    let custom_path = tmp_home_dir.path().join("custom_path");

    fs::create_dir(&custom_path).unwrap();

    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("setup")
        .arg("subnet")
        .arg("--path")
        .arg(&custom_path);

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Polygon Edge installation successful"));

    let file = fs::read_dir(&custom_path)
        .unwrap()
        .filter_map(|x| match x.ok() {
            Some(f) if f.path().ends_with("polygon-edge") => Some(f),
            _ => None,
        })
        .last()
        .unwrap();

    assert!(file.path().starts_with(&custom_path));

    Ok(())
}

#[test]
fn setup_subnet_install_edge_custom_path_env() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_home_dir = tempdir()?;
    let custom_path = tmp_home_dir.path().join("custom_path");

    fs::create_dir(&custom_path).unwrap();

    let mut cmd = Command::cargo_bin("topos")?;

    cmd.env("TOPOS_SETUP_POLYGON_EDGE_DIR", &custom_path)
        .arg("setup")
        .arg("subnet");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    assert!(result.contains("Polygon Edge installation successful"));

    let file = fs::read_dir(&custom_path)
        .unwrap()
        .filter_map(|x| match x.ok() {
            Some(f) if f.path().ends_with("polygon-edge") => Some(f),
            _ => None,
        })
        .last()
        .unwrap();

    assert!(file.path().starts_with(&custom_path));

    Ok(())
}
