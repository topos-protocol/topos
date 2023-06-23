pub mod error;
pub mod network;
pub mod node;
pub mod sequener;
pub mod setup;
pub mod subnet;
pub mod tce;

use clap::Parser;
use error::{ConfigError, InvalidType};
use figment::error::Kind;
use figment::providers::Env;
use figment::providers::Format;
use figment::providers::Serialized;
use figment::providers::Toml;
use figment::Figment;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

use network::NetworkConfig;
use node::NodeConfig;
use sequener::SequencerConfig;
use setup::SetupConfig;
use subnet::SubnetConfig;
use tce::TceConfig;

use crate::components::node::commands::{NodeCommand, NodeCommands};
#[cfg(feature = "tce")]
use crate::components::tce::commands::{TceCommand, TceCommands};
use crate::options;
use crate::options::{Opt, ToposCommand};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub tce: TceConfig,
    pub network: NetworkConfig,
    pub node: NodeConfig,
    pub sequencer: SequencerConfig,
    pub setup: SetupConfig,
    pub subnet: SubnetConfig,
}

impl Config {
    pub fn new(args: Opt) -> Result<Self, ConfigError> {
        let mut figment = Self::build_default_config();
        figment = Self::merge_config_with_cmd_args(figment, args);
        let config = Self::extract_valid_config(figment);

        let node_name = config.node.name.clone();

        let config_path = Self::create_node_config_file(node_name);
        let toml = toml::to_string(&config).unwrap();

        std::fs::write(config_path, toml).expect("Unable to write file");

        Ok(config)
    }

    /// We load the config from the config file and merge it with the command line arguments
    ///TODO: We should not use `Config::default` here, but check if a file exists or if the values
    /// needed are provided by the CLI. If not, we have to fail and/or create a new config file with
    /// the values needed to run it, and notify the user to fill out the empty values in the file
    pub fn load(args: Opt, node_name: String) -> Config {
        let config_path = Self::get_config_path(node_name);

        let mut figment = Figment::new()
            .merge(Serialized::defaults(Config::default()))
            .merge(Toml::file(config_path));

        figment = Self::merge_config_with_cmd_args(figment, args);

        Self::extract_valid_config(figment)
    }

    /// Build the initial config file to write to a file.
    /// We assume here that the file is not yet created
    fn build_default_config() -> Figment {
        Figment::new().merge(Serialized::defaults(Config::default()))
    }

    /// Get the path to the config file from the environment through `$TOPOS_HOME` or
    /// default to `/Users/USERNAME/.config/topos/node/NODE_NAME/config.toml`
    ///TODO: Adapt this to not only using `node` but also `subnet` and other hierachies
    fn create_node_config_file(node_name: String) -> PathBuf {
        let topos_home = match std::env::var("TOPOS_HOME") {
            Ok(path) => PathBuf::from(path).join("node").join(node_name),
            Err(_) => {
                let home_dir = dirs::home_dir().expect("Failed to get home directory");
                home_dir
                    .join(".config")
                    .join("topos")
                    .join("node")
                    .join(node_name)
            }
        };
        if !topos_home.exists() {
            std::fs::create_dir_all(&topos_home).expect("Could not create topos home directory");
        }

        topos_home.join("config.toml")
    }

    fn get_config_path(node_name: String) -> PathBuf {
        let topos_home = match std::env::var("TOPOS_HOME") {
            Ok(path) => PathBuf::from(path).join("node").join(node_name),
            Err(_) => {
                let home_dir = dirs::home_dir().expect("Failed to get home directory");
                home_dir
                    .join(".config")
                    .join("topos")
                    .join("node")
                    .join(node_name)
            }
        };

        if !topos_home.exists() {
            eprintln!("The requested config file: {}/config.rs was not found.\nPlease provide a valid node name or run topos node init --name NODENAME", topos_home.display());
            std::process::exit(1);
        }

        topos_home.join("config.toml")
    }

    /// Try to build a `Config` object from the `Figment` object
    /// and error out if values are missing or have the wrong type
    fn extract_valid_config(figment: Figment) -> Config {
        match figment.extract() {
            Ok(config) => config,
            Err(figment::Error {
                kind: Kind::MissingField(name),
                ..
            }) => {
                println!("Missing configuration value: {}", name);
                std::process::exit(1);
            }
            Err(figment::Error {
                kind: Kind::InvalidType(actual, expected),
                ..
            }) => {
                println!(
                    "Missing configuration value type, expecting {}, found {}",
                    expected, actual
                );
                std::process::exit(1);
            }

            Err(_) => panic!("TEST"),
        }
    }

    /// If a command line argument is provided, merge it with the config
    fn merge_config_with_cmd_args(figment: Figment, args: Opt) -> Figment {
        match args.commands {
            #[cfg(feature = "network")]
            ToposCommand::Network(ref cmd) => {
                figment.merge(Serialized::defaults(cmd).key("network"))
            }
            ToposCommand::Node(NodeCommand {
                ref subcommands, ..
            }) => match subcommands {
                Some(NodeCommands::Init(init_command)) => {
                    figment.merge(Serialized::defaults(init_command).key("node"))
                }
                Some(NodeCommands::Up(up_command)) => {
                    figment.merge(Serialized::defaults(up_command).key("node"))
                }
                _ => figment,
            },
            #[cfg(feature = "sequencer")]
            ToposCommand::Sequencer(ref cmd) => {
                figment.merge(Serialized::defaults(cmd).key("sequencer"))
            }
            #[cfg(feature = "setup")]
            ToposCommand::Setup(ref cmd) => figment.merge(Serialized::defaults(cmd).key("setup")),
            #[cfg(feature = "subnet")]
            ToposCommand::Subnet(ref cmd) => figment.merge(Serialized::defaults(cmd).key("subnet")),
            #[cfg(feature = "tce")]
            ToposCommand::Tce(TceCommand {
                ref subcommands, ..
            }) => match subcommands {
                Some(TceCommands::Run(run_command)) => {
                    figment.merge(Serialized::defaults(run_command).key("tce"))
                }
                Some(TceCommands::Keys(key_command)) => {
                    figment.merge(Serialized::defaults(key_command).key("tce"))
                }
                _ => figment,
            },
            ToposCommand::Doctor => figment,
        }
    }
}
