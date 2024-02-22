use prometheus::{
    register_histogram_with_registry, register_int_counter_with_registry,
    register_int_gauge_with_registry, Histogram, IntCounter, IntGauge,
};

use lazy_static::lazy_static;

use crate::TOPOS_METRIC_REGISTRY;

lazy_static! {
    pub static ref STORAGE_COMMAND_CHANNEL_CAPACITY_TOTAL: IntCounter =
        register_int_counter_with_registry!(
            "storage_command_channel_capacity_total",
            "Number of time the storage command channel was at capacity.",
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref STORAGE_PENDING_CERTIFICATE_EXISTENCE_LATENCY: Histogram =
        register_histogram_with_registry!(
            "storage_pending_certificate_existence_latency",
            "Latency of the pending certificate existance check.",
            vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0],
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref STORAGE_ADDING_PENDING_CERTIFICATE_LATENCY: Histogram =
        register_histogram_with_registry!(
            "storage_adding_pending_certificate_latency",
            "Latency of adding a pending certificate.",
            vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0],
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref STORAGE_PENDING_POOL_COUNT: IntGauge = register_int_gauge_with_registry!(
        "storage_pending_pool_count",
        "Number of certificates in the pending pool.",
        TOPOS_METRIC_REGISTRY
    )
    .unwrap();
    pub static ref STORAGE_PRECEDENCE_POOL_COUNT: IntGauge = register_int_gauge_with_registry!(
        "storage_precedence_pool_count",
        "Number of certificates in the precedence pool.",
        TOPOS_METRIC_REGISTRY
    )
    .unwrap();
}
