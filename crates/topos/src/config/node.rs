use std::path::Path;

use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};

use serde::{Deserialize, Serialize};

use crate::components::node::commands::Up;
use crate::config::{base::BaseConfig, sequencer::SequencerConfig, tce::TceConfig, Config};

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct NodeConfig {
    pub(crate) base: BaseConfig,
    pub(crate) tce: TceConfig,
    pub(crate) sequencer: SequencerConfig,
}

impl Config for NodeConfig {
    type Command = Up;
    type Output = NodeConfig;

    fn profile(&self) -> String {
        "default".to_string()
    }

    fn load_from_file(figment: Figment, home: &Path) -> Figment {
        let home = home.join("config.toml");

        figment.merge(Toml::file(home))
    }

    fn load_context(figment: Figment) -> Result<Self::Output, figment::Error> {
        figment.extract()
    }
}
