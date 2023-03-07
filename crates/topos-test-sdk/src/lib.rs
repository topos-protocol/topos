pub mod certificates;

pub mod storage;

pub mod constants {
    use proc_macro_sdk::generate_certificate_ids;
    use proc_macro_sdk::generate_source_subnet_ids;
    use proc_macro_sdk::generate_target_subnet_ids;
    use topos_core::uci::CertificateId;

    generate_source_subnet_ids!(100..150);
    generate_target_subnet_ids!(150..200);

    // Certificate range is 0..100
    pub const PREV_CERTIFICATE_ID: CertificateId = CertificateId::from_array([0u8; 32]);
    generate_certificate_ids!(1..100);
}
