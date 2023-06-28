use regex::Regex;

#[cfg(test)]
pub fn sanitize_config_folder_path(cmd_out: &str) -> String {
    // Sanitize the result here:
    // When run locally, we get /Users/<username>/.config/topos
    // When testing on the CI, we get /home/runner/.config/topos
    let pattern = Regex::new(r"\[default: .+?/.config/topos\]").unwrap();
    pattern
        .replace(cmd_out, "[default: /home/runner/.config/topos]")
        .to_string()
}
