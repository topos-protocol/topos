use std::process::Command;

use assert_cmd::prelude::*;
use regex::Regex;

#[test]
fn network_spam_help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("network").arg("spam").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    // Sanitize the result here:
    // When run locally, we get /Users/<username>/.config/topos
    // When testing on the CI, we get /home/runner/.config/topos
    let pattern = Regex::new(r"\[default: .+?/.config/topos\]").unwrap();
    let sanitized_result = pattern.replace(&result, "[default: /home/runner/.config/topos]");

    insta::assert_snapshot!(sanitized_result);

    Ok(())
}
