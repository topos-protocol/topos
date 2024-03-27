use std::path::{Path, PathBuf};

use figment::{
    providers::{Format, Toml},
    Figment,
};

use serde::{Deserialize, Serialize};
use topos_wallet::SecretManager;
use tracing::{debug, error};

use crate::{
    base::BaseConfig,
    certificate_producer::CertificateProducerConfig,
    edge::{EdgeBinConfig, EdgeConfig},
    load_config,
    tce::TceConfig,
    Config,
};

#[derive(clap::ValueEnum, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeRole {
    Validator,
    CertificateProducer,
    FullNode,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NodeConfig {
    pub base: BaseConfig,
    pub tce: Option<TceConfig>,
    pub certificate_producer: Option<CertificateProducerConfig>,
    pub edge: Option<EdgeConfig>,

    #[serde(skip)]
    pub home_path: PathBuf,

    #[serde(skip)]
    pub node_path: PathBuf,

    #[serde(skip)]
    pub genesis_path: PathBuf,

    #[serde(skip)]
    pub edge_bin: Option<EdgeBinConfig>,
}

impl NodeConfig {
    /// Try to create a new node config struct from the given home path and node name.
    /// It expects a config file to be present in the node's folder.
    ///
    /// This `config.toml` can be generated using: `topos node init` command
    pub fn try_from<S: Serialize>(
        home_path: &Path,
        node_name: &str,
        config: Option<&S>,
    ) -> Result<Self, std::io::Error> {
        let node_path = home_path.join("node").join(node_name);
        let config_path = node_path.join("config.toml");

        // TODO: Move this to `topos-node` when migrated
        if !Path::new(&config_path).exists() {
            error!(
                "Please run 'topos node init --name {node_name}' to create a config file first \
                 for {node_name}."
            );
            std::process::exit(1);
        }

        Ok(Self::build_config(node_path, home_path, config))
    }

    /// Create a new node config struct from the given home path and node name.
    ///
    /// It doesn't check the existence of the config file.
    /// It's useful for creating a config file for a new node, relying on the default values.
    pub fn create<S: Serialize>(home_path: &Path, node_name: &str, config: Option<&S>) -> Self {
        let node_path = home_path.join("node").join(node_name);

        Self::build_config(node_path, home_path, config)
    }

    /// Common function to build a node config struct from the given home path and node name.
    fn build_config<S: Serialize>(
        node_path: PathBuf,
        home_path: &Path,
        config: Option<&S>,
    ) -> Self {
        let node_folder = node_path.as_path();
        let base = load_config::<BaseConfig, _>(node_folder, config);

        // Load genesis pointed by the local config
        let genesis_path = home_path
            .join("subnet")
            .join(base.subnet.clone())
            .join("genesis.json");

        let mut config = NodeConfig {
            node_path: node_path.to_path_buf(),
            genesis_path,
            home_path: home_path.to_path_buf(),
            base: base.clone(),
            certificate_producer: base
                .need_certificate_producer()
                .then(|| load_config::<CertificateProducerConfig, ()>(node_folder, None)),
            tce: base
                .need_tce()
                .then(|| load_config::<TceConfig, ()>(node_folder, None)),
            edge_bin: base
                .need_edge()
                .then(|| load_config::<EdgeBinConfig, _>(node_folder, config)),
            edge: base
                .need_edge()
                .then(|| load_config::<EdgeConfig, ()>(node_folder, None)),
        };

        // Make the TCE DB path relative to the folder
        if let Some(config) = config.tce.as_mut() {
            config.db_path = node_folder.join(&config.db_path);
            debug!(
                "Maked TCE DB path relative to the node folder -> {:?}",
                config.db_path
            );
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

impl From<&NodeConfig> for SecretManager {
    fn from(val: &NodeConfig) -> Self {
        match val.base.secrets_config.as_ref() {
            Some(secrets_config) => SecretManager::from_aws(secrets_config),
            None => SecretManager::from_fs(val.node_path.clone()),
        }
    }
}
