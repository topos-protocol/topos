use topos_core::uci::Certificate;

use rstest::*;

use crate::constants::PREV_CERTIFICATE_ID;
use crate::constants::SOURCE_SUBNET_ID_1;
use crate::constants::TARGET_SUBNET_ID_1;

#[fixture]
pub fn create_certificate_chain(
    #[default(SOURCE_SUBNET_ID_1)] source_subnet: topos_core::uci::SubnetId,
    #[default(TARGET_SUBNET_ID_1)] target_subnet: topos_core::uci::SubnetId,
    #[default(1)] number: usize,
) -> Vec<Certificate> {
    let mut certificates = Vec::new();
    let mut parent = None;

    for _ in 0..number {
        let cert = Certificate::new(
            parent.take().unwrap_or(*PREV_CERTIFICATE_ID.as_array()),
            source_subnet,
            Default::default(),
            Default::default(),
            &[target_subnet],
            0,
            Default::default(),
        )
        .unwrap();
        parent = Some(*cert.id.as_array());
        certificates.push(cert);
    }

    certificates
}
