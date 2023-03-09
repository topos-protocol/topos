use std::collections::HashMap;
use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_test_sdk::constants::*;

/// Generate and assign nb_cert number of certificates to existing subnets
/// Could be different number of certificates per subnet
pub fn generate_cert(
    subnets: &Vec<SubnetId>,
    number_of_certificates_per_subnet: usize,
) -> HashMap<SubnetId, Vec<Certificate>> {
    let mut nonce_state: HashMap<SubnetId, CertificateId> = HashMap::new();
    let mut result: HashMap<SubnetId, Vec<Certificate>> = HashMap::new();
    for subnet in subnets {
        result.insert(*subnet, Vec::new());
    }

    // Initialize the genesis of all subnets
    for subnet in subnets {
        nonce_state.insert(*subnet, PREV_CERTIFICATE_ID);
    }

    let mut gen_cert = |selected_subnet: SubnetId| -> Certificate {
        let last_cert_id = nonce_state.get_mut(&selected_subnet).unwrap();
        // Add one cross chain transaction for every other subnet
        let target_subnets = subnets
            .iter()
            .filter(|sub| *sub != &selected_subnet)
            .cloned()
            .collect::<Vec<_>>();

        let gen_cert = Certificate::new(
            *last_cert_id,
            selected_subnet,
            Default::default(),
            Default::default(),
            &target_subnets,
            0,
            Vec::new(),
        )
        .expect("valid new certificate");
        *last_cert_id = gen_cert.id;
        gen_cert
    };

    for selected_subnet in subnets {
        for _ in 0..number_of_certificates_per_subnet {
            let cert = gen_cert(*selected_subnet);
            result
                .entry(*selected_subnet)
                .or_insert(Vec::new())
                .push(cert);
        }
    }
    result
}
