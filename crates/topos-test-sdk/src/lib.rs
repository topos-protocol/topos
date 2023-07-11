pub mod certificates;

#[cfg(feature = "tce")]
pub mod tce;

pub mod networking;
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

#[macro_export]
macro_rules! wait_for_event {
    ($node:expr, matches: $( $pattern:pat_param )|+ $( if $guard: expr )?, $error_msg:expr) => {
        wait_for_event!($node, matches: $( $pattern )|+ $( if $guard )?, $error_msg, 100);
    };

    ($node:expr, matches: $( $pattern:pat_param )|+ $( if $guard: expr )?, $error_msg:expr, $timeout:expr) => {
        let assertion = async {
            while let Some(event) = $node.await {
                if matches!(event, $( $pattern )|+ $( if $guard )?) {
                    break;
                }
            }
        };

        if let Err(_) = tokio::time::timeout(std::time::Duration::from_millis($timeout), assertion).await
        {
            panic!("Timed out waiting ({}ms) for event: {}", $timeout, $error_msg);
        }
    };
}
