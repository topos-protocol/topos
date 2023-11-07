use std::path::Path;

use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use crate::components::node::commands::Init;
use crate::config::node::NodeRole;
use crate::config::Config;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct BaseConfig {
    #[serde(default = "default_name")]
    pub name: String,

    #[serde(default = "default_role")]
    pub role: NodeRole,

    #[serde(default = "default_subnet_id")]
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

fn default_subnet_id() -> String {
    "topos".to_string()
}

fn default_secrets_config() -> Option<String> {
    None
}

impl BaseConfig {
    pub fn need_tce(&self) -> bool {
        self.subnet_id == "topos"
    }

    pub fn need_sequencer(&self) -> bool {
        matches!(self.role, NodeRole::Sequencer)
    }

    pub fn need_edge(&self) -> bool {
        true
    }
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
