use std::process::Command;

fn main() {
    // Set TOPOS_VERSION to HEAD short commit hash if None
    if std::option_env!("TOPOS_VERSION").is_none() {
        let output = Command::new("git")
            .args(&["rev-parse", "--short", "HEAD"])
            .output()
            .expect("failed to access the HEAD commit hash");

        let git_hash = String::from_utf8(output.stdout).unwrap();

        println!("cargo:rustc-env=TOPOS_VERSION={}", git_hash);
    }
}
