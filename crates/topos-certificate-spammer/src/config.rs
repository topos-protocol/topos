#[derive(Clone, Debug)]
pub struct CertificateSpammerConfig {
    pub target_nodes: Option<Vec<String>>,
    pub target_nodes_path: Option<String>,
    pub local_key_seed: u64,
    pub cert_per_batch: u64,
    pub nb_subnets: u8,
    pub nb_batches: Option<u64>,
    pub batch_interval: u64,
    pub target_subnets: Option<Vec<String>>,
    pub benchmark: bool,
    pub hosts: Option<String>,
    pub number: Option<u32>,
}
