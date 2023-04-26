use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(about = "Run a test topos certificate spammer to send test certificates to the network")]
pub struct Spam {
    /// The target node api endpoint
    #[clap(long, env = "TARGET_NODE")]
    pub target_node: Option<String>,
    /// Path to file with list of target nodes
    #[clap(long, env = "TARGET_NODES_PATH")]
    pub target_nodes_path: Option<String>,
    /// The number of certificate emitted per batch
    #[clap(long, default_value_t = 1, env = "CERT_PER_BATCH")]
    pub cert_per_batch: usize,
    /// Time interval between batches in seconds
    #[clap(long, default_value_t = 1, env = "BATCH_TIME_INTERVAL")]
    pub batch_time_interval: u64,
    /// The number of subnets that are emitter
    #[clap(long, default_value_t = 5)]
    pub nb_subnets: usize,
    /// The threshold of byzantine among the subnets between 0. (all correct) and 1. (all byzantine)
    #[clap(long, default_value_t = 0.)]
    pub byzantine_threshold: f32,
    /// The number of nodes to whom we emit the certificate to broadcast
    /// Ideally should work with one node to have the liveness property
    #[clap(long, default_value_t = 1)]
    pub node_per_cert: usize,
    /// Socket of the opentelemetry agent endpoint
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_AGENT")]
    pub otlp_agent: Option<String>,
    /// Otlp service name
    /// If not provided open telemetry will not be used
    #[arg(long, env = "TOPOS_OTLP_SERVICE_NAME")]
    pub otlp_service_name: Option<String>,
}

impl Spam {}
