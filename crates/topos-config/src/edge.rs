use crate::{edge::command::CommandConfig, Config};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::ExitStatus,
};
use tokio::{spawn, task::JoinHandle};
use tracing::{error, info};

use self::command::BINARY_NAME;

// TODO: Provides the default arguments here
// Serde `flatten` and `default` doesn't work together yet
// https://github.com/serde-rs/serde/issues/1626
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct EdgeConfig {
    #[serde(flatten)]
    pub args: HashMap<String, String>,
}

impl Config for EdgeConfig {
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

    fn profile() -> String {
        "edge".to_string()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct EdgeBinConfig {
    pub edge_path: PathBuf,
}

impl EdgeBinConfig {
    pub fn binary_path(&self) -> PathBuf {
        self.edge_path.join(BINARY_NAME)
    }
}

impl Config for EdgeBinConfig {
    type Output = EdgeBinConfig;

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

    fn profile() -> String {
        "edge".to_string()
    }
}
pub mod command;

pub fn generate_edge_config(
    edge_path: PathBuf,
    config_path: PathBuf,
) -> JoinHandle<Result<ExitStatus, std::io::Error>> {
    // Create the Polygon Edge config
    info!("Generating the configuration at {config_path:?}");
    info!("Polygon-edge binary located at: {edge_path:?}");
    spawn(async move {
        CommandConfig::new(edge_path)
            .init(&config_path)
            .spawn()
            .await
            .map_err(|e| {
                error!("Failed to generate the edge configuration: {e:?}");
                e
            })
    })
}
