pub(crate) mod base;
pub(crate) mod edge;
pub mod genesis;
pub mod node;
pub mod sequencer;
pub mod tce;

use std::path::Path;

use figment::providers::Serialized;
use figment::{error::Kind, Figment};
use serde::Serialize;

pub trait Config: Serialize {
    /// The configuration type returned (should be Self).
    type Output;

    /// Load the configuration from a file or multiple files.
    /// The home is the directory where the configuration files are located.
    /// For node, it is the `node` directory in the $TOPOS_HOME directory.
    fn load_from_file(figment: Figment, home: &Path) -> Figment;

    /// Load the configuration from the context.
    /// Trying to extract the configuration from the figment context.
    fn load_context(figment: Figment) -> Result<Self::Output, figment::Error>;

    /// Return the profile name of the configuration to be used
    /// when generating the file.
    fn profile() -> String;

    /// Convert the configuration to a TOML table.
    fn to_toml(&self) -> Result<toml::Table, toml::ser::Error> {
        let mut config_toml = toml::Table::new();

        let config = toml::Table::try_from(self)?;

        // Flatten the top level
        for (profile, content) in config {
            config_toml.insert(profile, content);
        }

        Ok(config_toml)
    }

    /// Main function to load the configuration.
    /// It will load the configuration from the file and an optional existing struct (if any)
    /// and then extract the configuration from the context in order to build the Config.
    /// The Config is then returned or an error if the configuration is not valid.
    fn load<S: Serialize>(home: &Path, config: Option<S>) -> Result<Self::Output, figment::Error> {
        let mut figment = Figment::new();

        figment = Self::load_from_file(figment, home);

        if let Some(config) = config {
            figment = figment.merge(Serialized::from(config, Self::profile()))
        }

        Self::load_context(figment)
    }
}

pub(crate) fn load_config<T: Config, S: Serialize>(
    node_path: &Path,
    config: Option<S>,
) -> T::Output {
    match T::load(node_path, config) {
        Ok(config) => config,
        Err(figment::Error {
            kind: Kind::MissingField(name),
            ..
        }) => {
            println!("Missing field: {}", name);
            std::process::exit(1);
        }
        Err(e) => {
            println!("Failed to load config: {e}");
            std::process::exit(1);
        }
    }
}
