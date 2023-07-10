#[cfg(feature = "node")]
pub(crate) mod base;
#[cfg(feature = "node")]
pub(crate) mod node;
#[cfg(feature = "sequencer")]
pub(crate) mod sequencer;
#[cfg(feature = "tce")]
pub(crate) mod tce;

use std::path::Path;

use figment::{
    error::Kind,
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::Serialize;

pub(crate) trait Config: Serialize {
    /// The command line command to load the configuration.
    type Command: Serialize;
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
    fn profile(&self) -> String;

    /// Convert the configuration to a TOML table.
    fn to_toml(&self) -> Result<toml::Table, toml::ser::Error> {
        toml::Table::try_from(self)
    }

    /// Load the configuration from the command line command.
    fn load_from_command(figment: Figment, command: Self::Command) -> Figment {
        figment.merge(Serialized::defaults(command))
    }

    /// Main function to load the configuration.
    /// It will load the configuration from the file and the command line (if any)
    /// and then extract the configuration from the context in order to build the Config.
    /// The Config is then returned or an error if the configuration is not valid.
    fn load(home: &Path, command: Option<Self::Command>) -> Result<Self::Output, figment::Error> {
        let mut figment = Figment::new();

        figment = Self::load_from_file(figment, home);

        if let Some(command) = command {
            figment = Self::load_from_command(figment, command);
        }

        Self::load_context(figment)
    }
}

#[allow(dead_code)]
pub(crate) fn load_config<T: Config>(node_path: &Path, command: Option<T::Command>) -> T::Output {
    match T::load(node_path, command) {
        Ok(config) => config,
        Err(figment::Error {
            kind: Kind::MissingField(name),
            ..
        }) => {
            println!("Missing field: {}", name);
            std::process::exit(1);
        }
        _ => {
            println!("Failed to load config");
            std::process::exit(1);
        }
    }
}

#[allow(dead_code)]
pub(crate) fn insert_into_toml<T: Config>(config_toml: &mut toml::Table, config: T) {
    config_toml.insert(
        config.profile(),
        toml::Value::Table(config.to_toml().expect("failed to convert config to toml")),
    );
}
