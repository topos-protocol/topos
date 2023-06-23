use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NodeConfig {
    /// Name to identify your node
    #[serde(default = "default_name")]
    pub name: String,

    /// Role of your node
    #[serde(default = "default_role")]
    pub role: String,

    /// Subnet of your node
    #[serde(default = "default_subnet")]
    pub subnet: String,
}

fn default_name() -> String {
    "default".to_string()
}

fn default_role() -> String {
    "validator".to_string()
}

fn default_subnet() -> String {
    "topos".to_string()
}

impl Default for NodeConfig {
    fn default() -> Self {
        NodeConfig {
            name: default_name(),
            role: default_role(),
            subnet: default_subnet(),
        }
    }
}
