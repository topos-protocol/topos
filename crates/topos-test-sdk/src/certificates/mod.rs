use topos_core::uci::Certificate;

use crate::constants::PREV_CERTIFICATE_ID;

pub fn create_certificate_chain(
    source_subnet: topos_core::uci::SubnetId,
    target_subnet: topos_core::uci::SubnetId,
    number: usize,
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
