use std::path::{Path, PathBuf};

use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};

use serde::{Deserialize, Serialize};

use crate::components::node::{self, commands::Up};
use crate::config::{
    base::BaseConfig, edge::EdgeConfig, sequencer::SequencerConfig, tce::TceConfig, Config,
};

use super::load_config;

#[derive(clap::ValueEnum, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    Validator,
    Sequencer,
    FullNode,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct NodeConfig {
    pub(crate) base: BaseConfig,
    pub(crate) tce: Option<TceConfig>,
    pub(crate) sequencer: Option<SequencerConfig>,
    #[serde(rename = "subnet")]
    pub(crate) edge: Option<EdgeConfig>,
}

impl NodeConfig {
    pub fn new(from: &Path, cmd: Option<node::commands::Init>) -> Self {
        let base = load_config::<BaseConfig>(from, cmd);

        Self {
            base: base.clone(),
            sequencer: base
                .need_sequencer()
                .then(|| load_config::<SequencerConfig>(from, None)),
            tce: base
                .need_tce()
                .then(|| load_config::<TceConfig>(from, None)),
            edge: base
                .need_edge()
                .then(|| load_config::<EdgeConfig>(from, None)),
        }
    }
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
