use std::path::{Path, PathBuf};

use crate::config::Config;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::components::subnet::commands::Run;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EdgeConfig {
    /// SubnetId of the local subnet node, hex encoded 32 bytes starting with 0x
    pub subnet_id: Option<String>,

    // Core contract address
    #[serde(default = "default_subnet_contract_address")]
    pub subnet_contract_address: String,

    /// Base Uri of TCE node to call grpc service api
    #[serde(default = "default_base_tce_api_url")]
    pub base_tce_api_url: String,

    /// Polygon subnet node data dir, containing `consensus/validator.key`, e.g. `../test-chain-1`
    #[serde(default = "default_subnet_data_dir")]
    pub subnet_data_dir: PathBuf,
}

fn default_subnet_contract_address() -> String {
    "0x0000000000000000000000000000000000000000".to_string()
}

fn default_base_tce_api_url() -> String {
    "http://[::1]:1340".to_string()
}

fn default_subnet_data_dir() -> PathBuf {
    PathBuf::from("../test-chain-1")
}

impl Config for EdgeConfig {
    type Command = Run;

    type Output = EdgeConfig;

    fn load_from_file(figment: Figment, home: &Path) -> Figment {
        let home = home.join("config.toml");

        let edge = Figment::new()
            .merge(Toml::file(home).nested())
            .select("edge");

        figment.merge(edge)
    }

    fn load_context(figment: Figment) -> Result<Self::Output, figment::Error> {
        figment.extract()
    }

    fn profile(&self) -> String {
        "edge".to_string()
    }
}
