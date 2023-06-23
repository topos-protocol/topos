use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct SetupConfig {
    /// Installation directory path for Polygon Edge binary.
    /// If not provided, Polygon Edge binary will be installed to the current directory
    #[serde(default = "default_path")]
    pub path: PathBuf,

    /// Polygon Edge release version. If not provided, latest release version will be installed
    pub release: Option<String>,

    /// Polygon Edge Github repository
    #[serde(default = "default_repository")]
    pub repository: String,

    /// List all available Polygon Edge release versions without installation
    pub list_releases: bool,
}

fn default_path() -> PathBuf {
    PathBuf::from(".")
}

fn default_repository() -> String {
    "topos-network/polygon-edge".to_string()
}

impl Default for SetupConfig {
    fn default() -> Self {
        SetupConfig {
            path: default_path(),
            release: None,
            repository: default_repository(),
            list_releases: false,
        }
    }
}
