use std::path::Path;

use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::components::node::commands::Init;
use crate::config::node::NodeRole;
use crate::config::Config;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BaseConfig {
    #[serde(default = "default_name")]
    pub name: String,

    #[serde(default = "default_role")]
    pub role: NodeRole,

    #[serde(default = "default_subnet")]
    pub subnet_id: String,

    #[serde(default = "default_secrets_config")]
    pub secrets_config: Option<String>,
}

fn default_name() -> String {
    "default".to_string()
}

fn default_role() -> NodeRole {
    NodeRole::Validator
}

fn default_subnet() -> String {
    "topos".to_string()
}

fn default_secrets_config() -> Option<String> {
    None
}

impl Config for BaseConfig {
    type Command = Init;

    type Output = Self;

    fn load_from_file(figment: Figment, home: &Path) -> Figment {
        let home = home.join("config.toml");

        let base = Figment::new()
            .merge(Toml::file(home).nested())
            .select("base");

        figment.merge(base)
    }

    fn load_context(figment: Figment) -> Result<Self::Output, figment::Error> {
        figment.extract()
    }

    fn profile(&self) -> String {
        "base".to_string()
    }
}
