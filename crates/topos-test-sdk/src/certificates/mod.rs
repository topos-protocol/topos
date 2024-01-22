use rstest::*;
use std::collections::HashMap;

use topos_core::{
    types::{
        stream::CertificateSourceStreamPosition, stream::Position, CertificateDelivered,
        ProofOfDelivery,
    },
    uci::{Certificate, CertificateId, SubnetId, INITIAL_CERTIFICATE_ID},
};

use crate::constants::PREV_CERTIFICATE_ID;
use crate::constants::SOURCE_SUBNET_ID_1;
use crate::constants::TARGET_SUBNET_ID_1;

#[fixture]
pub fn create_certificate(
    #[default(SOURCE_SUBNET_ID_1)] source_subnet: SubnetId,
    #[default(&[TARGET_SUBNET_ID_1])] target_subnets: &[SubnetId],
    #[default(None)] previous_certificate_id: Option<CertificateId>,
) -> Certificate {
    Certificate::new_with_default_fields(
        previous_certificate_id.unwrap_or(INITIAL_CERTIFICATE_ID),
        source_subnet,
        target_subnets,
    )
    .unwrap()
}

#[fixture]
pub fn create_certificate_at_position(
    #[default(Position::ZERO)] position: Position,
    create_certificate: Certificate,
) -> CertificateDelivered {
    let certificate_id = create_certificate.id;
    let subnet_id = create_certificate.source_subnet_id;

    CertificateDelivered {
        certificate: create_certificate,
        proof_of_delivery: ProofOfDelivery {
            certificate_id,
            delivery_position: CertificateSourceStreamPosition {
                subnet_id,
                position,
            },
            readies: vec![],
            threshold: 0,
        },
    }
}

#[fixture]
pub fn create_certificate_chain(
    #[default(SOURCE_SUBNET_ID_1)] source_subnet: topos_core::uci::SubnetId,
    #[default(&[TARGET_SUBNET_ID_1])] target_subnets: &[topos_core::uci::SubnetId],
    #[default(1)] number: usize,
) -> Vec<CertificateDelivered> {
    let mut certificates = Vec::new();
    let mut parent = None;

    for i in 0..number {
        let cert = Certificate::new_with_default_fields(
            parent.take().unwrap_or(*PREV_CERTIFICATE_ID.as_array()),
            source_subnet,
            target_subnets,
        )
        .unwrap();
        parent = Some(*cert.id.as_array());
        let id = cert.id;
        certificates.push(CertificateDelivered {
            certificate: cert,
            proof_of_delivery: ProofOfDelivery {
                certificate_id: id,
                delivery_position: CertificateSourceStreamPosition {
                    subnet_id: source_subnet,
                    position: i.try_into().unwrap(),
                },
                readies: Vec::new(),
                threshold: 0,
            },
        });
    }

    certificates
}

/// Generate and assign nb_cert number of certificates to existing subnets
/// Could be different number of certificates per subnet
pub fn create_certificate_chains(
    subnets: &[SubnetId],
    number_of_certificates_per_subnet: usize,
) -> HashMap<SubnetId, Vec<CertificateDelivered>> {
    let mut result = HashMap::new();

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
