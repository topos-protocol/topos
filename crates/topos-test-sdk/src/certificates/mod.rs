use std::collections::HashMap;

use topos_core::uci::Certificate;
use topos_core::uci::SubnetId;

use rstest::*;

use crate::constants::PREV_CERTIFICATE_ID;
use crate::constants::SOURCE_SUBNET_ID_1;
use crate::constants::TARGET_SUBNET_ID_1;

#[fixture]
pub fn create_certificate_chain(
    #[default(SOURCE_SUBNET_ID_1)] source_subnet: topos_core::uci::SubnetId,
    #[default(&[TARGET_SUBNET_ID_1])] target_subnets: &[topos_core::uci::SubnetId],
    #[default(1)] number: usize,
) -> Vec<Certificate> {
    let mut certificates = Vec::new();
    let mut parent = None;

    for _ in 0..number {
        let cert = Certificate::new_with_default_fields(
            parent.take().unwrap_or(*PREV_CERTIFICATE_ID.as_array()),
            source_subnet,
            target_subnets,
        )
        .unwrap();
        parent = Some(*cert.id.as_array());
        certificates.push(cert);
    }

    certificates
}

/// Generate and assign nb_cert number of certificates to existing subnets
/// Could be different number of certificates per subnet
pub fn create_certificate_chains(
    subnets: &[SubnetId],
    number_of_certificates_per_subnet: usize,
) -> HashMap<SubnetId, Vec<Certificate>> {
    let mut result: HashMap<SubnetId, Vec<Certificate>> = HashMap::new();

    subnets.iter().for_each(|subnet| {
        let targets = subnets
            .iter()
            .filter(|sub| *sub != subnet)
            .copied()
            .collect::<Vec<_>>();

        let certs =
            create_certificate_chain(*subnet, targets.as_ref(), number_of_certificates_per_subnet);
        result.entry(*subnet).or_insert(certs);
    });

    result
}
