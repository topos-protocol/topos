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
        toml::Table::try_from(self)
    }

    /// Main function to load the configuration.
    /// It will load the configuration from the file and the command line (if any)
    /// and then extract the configuration from the context in order to build the Config.
    /// The Config is then returned or an error if the configuration is not valid.
    fn load<S: Serialize>(home: &Path, command: Option<S>) -> Result<Self::Output, figment::Error> {
        let mut figment = Figment::new();

        figment = Self::load_from_file(figment, home);

        if let Some(command) = command {
            figment = figment.merge(Serialized::from(command, Self::profile()))
        }

        Self::load_context(figment)
    }
}

pub(crate) fn load_config<T: Config, S: Serialize>(
    node_path: &Path,
    command: Option<S>,
) -> T::Output {
    match T::load(node_path, command) {
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

pub fn insert_into_toml<T: Config>(config_toml: &mut toml::Table, config: T) {
    let full = config.to_toml().expect("failed to convert config to toml");

    // Flatten the top level
    for (profile, content) in full {
        config_toml.insert(profile, content);
    }
}
