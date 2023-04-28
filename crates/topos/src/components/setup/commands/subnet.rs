use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(about = "Install Polygon Edge node binary")]
pub struct Subnet {
    /// Installation directory path for Polygon Edge binary.
    /// If not provided, Polygon Edge binary will be installed to the current directory
    #[clap(long, env = "TOPOS_SETUP_POLYGON_EDGE_DIR", default_value = ".")]
    pub path: PathBuf,
    /// Polygon Edge release version. If not provided, latest release version will be installed
    #[arg(long, env = "TOPOS_SETUP_SUBNET_RELEASE")]
    pub release: Option<String>,
    /// Polygon Edge Github repository
    #[arg(
        long,
        env = "TOPOS_SETUP_SUBNET_REPOSITORY",
        default_value = "topos-network/polygon-edge"
    )]
    pub repository: String,
    /// List all available Polygon Edge release versions without installation
    #[arg(long, action)]
    pub list_releases: bool,
}

impl Subnet {}
