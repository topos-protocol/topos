use std::collections::HashMap;

use rand::Rng;
use topos_tce_broadcast::uci::{Certificate, CertificateId, SubnetId};

pub fn generate_cert(subnets: &Vec<SubnetId>, nb_cert: usize) -> Vec<Certificate> {
    let mut nonce_state: HashMap<SubnetId, CertificateId> = HashMap::new();
    // Initialize the genesis of all subnets
    for subnet in subnets {
        nonce_state.insert(subnet.to_string(), 0.to_string());
    }

    let mut rng = rand::thread_rng();
    let mut gen_cert = || -> Certificate {
        let selected_subnet = subnets[rng.gen_range(0..subnets.len())].clone();
        let last_cert_id = nonce_state.get_mut(&selected_subnet).unwrap();
        let gen_cert = Certificate::new(last_cert_id.clone(), selected_subnet, Default::default());
        *last_cert_id = gen_cert.cert_id.clone();
        gen_cert
    };

    (0..nb_cert).map(|_| gen_cert()).collect::<Vec<_>>()
}
