pub mod certificates;

#[cfg(feature = "tce")]
pub mod tce;

pub mod p2p;
pub mod sequencer;
pub mod storage;

pub mod constants {
    use proc_macro_sdk::generate_certificate_ids;
    use proc_macro_sdk::generate_source_subnet_ids;
    use proc_macro_sdk::generate_target_subnet_ids;
    use topos_core::uci::CertificateId;
    use topos_core::uci::CERTIFICATE_ID_LENGTH;

    generate_source_subnet_ids!(100..150);
    generate_target_subnet_ids!(150..200);

    // Certificate range is 0..100
    pub const PREV_CERTIFICATE_ID: CertificateId =
        CertificateId::from_array([0u8; CERTIFICATE_ID_LENGTH]);
    generate_certificate_ids!(1..100);
}
