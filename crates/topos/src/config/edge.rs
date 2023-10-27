use std::{collections::HashMap, path::Path};

use crate::config::Config;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

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

    fn profile(&self) -> String {
        "edge".to_string()
    }
}
