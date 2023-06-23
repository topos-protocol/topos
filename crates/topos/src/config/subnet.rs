use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct SubnetConfig {
    /// Installation directory path for Polygon Edge binary.
    #[serde(default = "default_path")]
    pub path: PathBuf,

    /// Polygon Edge command line arguments
    pub polygon_edge_arguments: Vec<String>,
}

fn default_path() -> PathBuf {
    PathBuf::from(".")
}

impl Default for SubnetConfig {
    fn default() -> Self {
        SubnetConfig {
            path: default_path(),
            polygon_edge_arguments: vec![],
        }
    }
}
