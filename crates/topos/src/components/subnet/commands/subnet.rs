use clap::{ArgGroup, Args, Parser};

use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(about = "Run Polygon Edge", trailing_var_arg = true)]
pub struct Run {
    /// Installation directory path for Polygon Edge binary.
    /// If not provided, Polygon Edge binary will be expected in the current directory
    #[arg(long, env = "TOPOS_POLYGON_EDGE_BIN_PATH", default_value = ".")]
    pub path: PathBuf,

    /// Polygon Edge command line arguments
    #[clap(allow_hyphen_values = true)]
    pub polygon_edge_arguments: Vec<String>,
}

impl Run {}
