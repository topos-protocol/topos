use prometheus::{register_int_counter_with_registry, IntCounter};

use lazy_static::lazy_static;

use crate::TOPOS_METRIC_REGISTRY;

lazy_static! {
    pub static ref API_GRPC_CERTIFICATE_RECEIVED_TOTAL: IntCounter =
        register_int_counter_with_registry!(
            "api_grpc_certificate_received_total",
            "Number of Certificates received from the gRPC API.",
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
}
