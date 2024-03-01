use std::process::Command;

const DEFAULT_VERSION: &str = "detached";

fn main() {
    // Set TOPOS_VERSION to HEAD short commit hash unless it's already set
    if std::option_env!("TOPOS_VERSION").is_none() {
        let output = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .expect("failed to access the HEAD commit hash");

        let git_hash = String::from_utf8(output.stdout).unwrap();

        let topos_version = if git_hash.is_empty() {
            DEFAULT_VERSION
        } else {
            git_hash.as_str()
        };

        println!("cargo:rustc-env=TOPOS_VERSION={topos_version}");
    }
}
