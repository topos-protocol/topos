use prometheus::{
    self, register_histogram_with_registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry, Histogram, IntCounter, IntCounterVec,
};

use lazy_static::lazy_static;

use crate::TOPOS_METRIC_REGISTRY;

lazy_static! {
    pub static ref P2P_EVENT_STREAM_CAPACITY_TOTAL: IntCounter =
        register_int_counter_with_registry!(
            "p2p_event_stream_capacity_total",
            "Number of time the p2p event stream was almost at capacity.",
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref P2P_DUPLICATE_MESSAGE_ID_RECEIVED_TOTAL: IntCounter =
        register_int_counter_with_registry!(
            "p2p_duplicate_message_id_received_total",
            "Number of time a duplicate message id was received.",
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref P2P_MESSAGE_RECEIVED_ON_GOSSIP_TOTAL: IntCounter =
        register_int_counter_with_registry!(
            "p2p_gossip_message_total",
            "Number of gossip message received.",
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref P2P_MESSAGE_RECEIVED_ON_ECHO_TOTAL: IntCounter =
        register_int_counter_with_registry!(
            "p2p_echo_message_total",
            "Number of echo message received.",
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref P2P_MESSAGE_RECEIVED_ON_READY_TOTAL: IntCounter =
        register_int_counter_with_registry!(
            "p2p_ready_message_total",
            "Number of ready message received.",
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref P2P_MESSAGE_SENT_ON_GOSSIPSUB_TOTAL: IntCounter =
        register_int_counter_with_registry!(
            "p2p_gossipsub_message_sent_total",
            "Number of gossipsub message sent.",
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref P2P_GOSSIP_BATCH_SIZE: Histogram = register_histogram_with_registry!(
        "p2p_gossip_batch_size",
        "Number of message sent in a gossip batch.",
        vec![1.0, 5.0, 10.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0],
        TOPOS_METRIC_REGISTRY
    )
    .unwrap();
    pub static ref P2P_MESSAGE_DESERIALIZE_FAILURE_TOTAL: IntCounterVec =
        register_int_counter_vec_with_registry!(
            "p2p_message_deserialize_failure_total",
            "Number of message deserialization failure.",
            &["topic"],
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
    pub static ref P2P_MESSAGE_SERIALIZE_FAILURE_TOTAL: IntCounterVec =
        register_int_counter_vec_with_registry!(
            "p2p_message_serialize_failure_total",
            "Number of message serialization failure.",
            &["topic"],
            TOPOS_METRIC_REGISTRY
        )
        .unwrap();
}
