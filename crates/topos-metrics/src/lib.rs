mod server;

use prometheus::{self, IntCounter};

use lazy_static::lazy_static;
use prometheus::register_int_counter;

pub use server::ServerBuilder;

lazy_static! {
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
