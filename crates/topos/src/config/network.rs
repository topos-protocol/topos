use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NetworkConfig {
    /// The target node api endpoint.
    /// Multiple nodes could be specified as comma separated list
    #[serde(default = "default_target_nodes")]
    pub target_nodes: Vec<String>,

    /// Path to json file with list of target nodes as alternative to `target_nodes`
    pub target_nodes_path: Option<String>,

    /// Seed for generation of local private signing keys and corresponding subnet ids.
    #[serde(default = "default_local_key_seed")]
    pub local_key_seed: u64,

    /// Certificates generated in one batch. Batch is generated every `batch-interval` milliseconds.
    #[serde(default = "default_cert_per_batch")]
    pub cert_per_batch: u8,

    /// Number of subnets to use for certificate generation. For every certificate subnet id will be picked randomly.
    #[serde(default = "default_nb_subnets")]
    pub nb_subnets: u8,

    /// Number of batches to generate before finishing execution.
    /// If not specified, batches will be generated indefinitely.
    #[serde(default = "default_nb_batches")]
    pub nb_batches: u64,

    /// Time interval in milliseconds between generated batches of certificates
    #[serde(default = "default_batch_interval")]
    pub batch_interval: u64,

    /// List of generated certificate target subnets. No target subnets by default.
    #[serde(default = "default_target_subnets")]
    pub target_subnets: Vec<String>,

    /// Socket of the opentelemetry agent endpoint.
    /// If not provided open telemetry will not be used
    pub otlp_agent: Option<String>,

    /// Otlp service name.
    /// If not provided open telemetry will not be used
    pub otlp_service_name: Option<String>,
}

fn default_target_nodes() -> Vec<String> {
    vec!["http://[::1]:1340,http://[::1]:1341".to_string()]
}

fn default_local_key_seed() -> u64 {
    1
}

fn default_cert_per_batch() -> u8 {
    1
}

fn default_nb_subnets() -> u8 {
    1
}

fn default_nb_batches() -> u64 {
    10
}

fn default_batch_interval() -> u64 {
    2000
}

fn default_target_subnets() -> Vec<String> {
    vec![
        "0x3bc19e36ff1673910575b6727a974a9abd80c9a875d41ab3e2648dbfb9e4b518,0xa00d60b2b408c2a14c5d70cdd2c205db8985ef737a7e55ad20ea32cc9e7c417c".to_string()
    ]
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            target_nodes: default_target_nodes(),
            target_nodes_path: None,
            local_key_seed: default_local_key_seed(),
            cert_per_batch: default_cert_per_batch(),
            nb_subnets: default_nb_subnets(),
            nb_batches: default_nb_batches(),
            batch_interval: default_batch_interval(),
            target_subnets: default_target_subnets(),
            otlp_agent: None,
            otlp_service_name: None,
        }
    }
}
