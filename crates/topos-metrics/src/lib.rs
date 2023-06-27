use prometheus::{
    self, register_histogram, register_int_gauge, Encoder, Histogram, IntCounter, IntGauge,
    TextEncoder,
};

use lazy_static::lazy_static;
use prometheus::register_int_counter;

lazy_static! {
    // p2p
    pub static ref P2P_EVENT_STREAM_CAPACITY: IntCounter = register_int_counter!(
        "p2p_event_stream_capacity",
        "Number of time the p2p event stream was almost at capacity."
    ).unwrap();

    pub static ref P2P_DUPLICATE_MESSAGE_ID_RECEIVED: IntCounter = register_int_counter!(
        "p2p_duplicate_message_id_received",
        "Number of time a duplicate message id was received."
    ).unwrap();

    pub static ref MESSAGE_RECEIVED_ON_GOSSIP: IntCounter =
        register_int_counter!("gossip_message_count", "Number of gossip message received.")
            .unwrap();
    pub static ref MESSAGE_RECEIVED_ON_ECHO: IntCounter =
        register_int_counter!("echo_message_count", "Number of echo message received.").unwrap();
    pub static ref MESSAGE_RECEIVED_ON_READY: IntCounter =
        register_int_counter!("ready_message_count", "Number of ready message received.").unwrap();
    pub static ref MESSAGE_SENT_ON_GOSSIPSUB: IntCounter = register_int_counter!(
        "gossipsub_message_sent_count",
        "Number of gossipsub message sent."
    )
    .unwrap();
    pub static ref P2P_GOSSIP_BATCH_SIZE: Histogram = register_histogram!(
        "p2p_gossip_batch_size",
        "Number of message sent in a gossip batch.",
        vec![1.0, 5.0, 10.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0]
    ).unwrap();

    // Storage
    pub static ref STORAGE_COMMAND_CHANNEL_CAPACITY: IntCounter = register_int_counter!(
        "storage_command_channel_capacity",
        "Number of time the storage command channel was at capacity."
    ).unwrap();

    pub static ref STORAGE_PENDING_CERTIFICATE_EXISTANCE_LATENCY: Histogram = register_histogram!(
        "storage_pending_certificate_existance_latency",
        "Latency of the pending certificate existance check.",
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]
    ).unwrap();

    pub static ref STORAGE_ADDING_PENDING_CERTIFICATE_LATENCY: Histogram = register_histogram!(
        "storage_adding_pending_certificate_latency",
        "Latency of adding a pending certificate.",
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]
    ).unwrap();

    // Double echo
    pub static ref DOUBLE_ECHO_COMMAND_CHANNEL_CAPACITY: IntCounter = register_int_counter!(
        "double_echo_command_channel_capacity",
        "Number of time the double echo command channel was at capacity."
    ).unwrap();

    pub static ref DOUBLE_ECHO_BUFFER_CAPACITY: IntCounter = register_int_counter!(
        "double_echo_buffer_capacity",
        "Number of time the double echo buffer was at capacity."
    ).unwrap();
    pub static ref DOUBLE_ECHO_CURRENT_BUFFER_SIZE: IntGauge = register_int_gauge!(
        "double_echo_current_buffer_size",
        "Current size of the double echo buffer."
    ).unwrap();

    pub static ref DOUBLE_ECHO_BUFFERED_MESSAGE_COUNT: IntGauge = register_int_gauge!(
        "double_echo_buffered_message_count",
        "Number of message buffered in the double echo buffer."
    ).unwrap();

    pub static ref DOUBLE_ECHO_BROADCAST_CREATED: IntCounter = register_int_counter!(
        "double_echo_broadcast_created",
        "Number of broadcast created."
    ).unwrap();

    pub static ref DOUBLE_ECHO_BROADCAST_FINISHED: IntCounter = register_int_counter!(
        "double_echo_broadcast_finished",
        "Number of broadcast finished."
    ).unwrap();

    pub static ref CERTIFICATE_RECEIVED: IntCounter =
        register_int_counter!("certificate_received", "Number of certificate received.").unwrap();
    pub static ref CERTIFICATE_RECEIVED_FROM_GOSSIP: IntCounter = register_int_counter!(
        "certificate_received_from_gossip",
        "Number of certificate received from gossip."
    )
    .unwrap();
    pub static ref CERTIFICATE_RECEIVED_FROM_API: IntCounter = register_int_counter!(
        "certificate_received_from_api",
        "Number of certificate received from api."
    )
    .unwrap();
    pub static ref CERTIFICATE_DELIVERED: IntCounter =
        register_int_counter!("certificate_delivered", "Number of certificate delivered.").unwrap();
}

pub fn gather_metrics() -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();

    // Gather the metrics.
    let metric_families = prometheus::gather();
    // Encode them to send.
    encoder.encode(&metric_families, &mut buffer).unwrap();

    String::from_utf8(buffer.clone()).unwrap()
}

pub fn init_metrics() {
    P2P_EVENT_STREAM_CAPACITY.reset();
    MESSAGE_RECEIVED_ON_GOSSIP.reset();
    MESSAGE_RECEIVED_ON_ECHO.reset();
    MESSAGE_RECEIVED_ON_READY.reset();
    MESSAGE_SENT_ON_GOSSIPSUB.reset();
    DOUBLE_ECHO_COMMAND_CHANNEL_CAPACITY.reset();
    DOUBLE_ECHO_BUFFER_CAPACITY.reset();
    DOUBLE_ECHO_CURRENT_BUFFER_SIZE.set(0);
    DOUBLE_ECHO_BUFFERED_MESSAGE_COUNT.set(0);
    CERTIFICATE_RECEIVED.reset();
    CERTIFICATE_RECEIVED_FROM_GOSSIP.reset();
    CERTIFICATE_RECEIVED_FROM_API.reset();
    CERTIFICATE_DELIVERED.reset();
}
