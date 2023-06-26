pub(crate) mod base;
pub(crate) mod node;
pub(crate) mod sequencer;
pub(crate) mod tce;

use std::path::Path;

use figment::{
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
