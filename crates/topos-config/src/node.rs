use std::path::Path;

use figment::{
    providers::{Format, Toml},
    Figment,
};

use serde::{Deserialize, Serialize};

use crate::{
    base::BaseConfig, edge::EdgeConfig, load_config, sequencer::SequencerConfig, tce::TceConfig,
    Config,
};

#[derive(clap::ValueEnum, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    Validator,
    Sequencer,
    FullNode,
}

#[derive(Serialize)]
struct Noop {}

#[derive(Serialize, Deserialize, Debug)]
pub struct NodeConfig {
    pub base: BaseConfig,
    pub tce: Option<TceConfig>,
    pub sequencer: Option<SequencerConfig>,
    pub edge: Option<EdgeConfig>,
}

impl NodeConfig {
    pub fn new<T: Serialize>(home: &Path, cmd: Option<T>) -> Self {
        let base = load_config::<BaseConfig, _>(home, cmd);

        let mut config = NodeConfig {
            base: base.clone(),
            sequencer: base
                .need_sequencer()
                .then(|| load_config::<SequencerConfig, Noop>(home, None)),
            tce: base
                .need_tce()
                .then(|| load_config::<TceConfig, Noop>(home, None)),
            edge: base
                .need_edge()
                .then(|| load_config::<EdgeConfig, Noop>(home, None)),
        };

        // Make the TCE DB path relative to the folder
        if let Some(config) = config.tce.as_mut() {
            config.db_path = home.join(&config.db_path);
        }

        config
    }
}

impl Config for NodeConfig {
    type Output = NodeConfig;

    fn load_from_file(figment: Figment, home: &Path) -> Figment {
        let home = home.join("config.toml");

        figment.merge(Toml::file(home))
    }

    fn load_context(figment: Figment) -> Result<Self::Output, figment::Error> {
        figment.extract()
    }

    fn profile() -> String {
        "default".to_string()
    }
}
