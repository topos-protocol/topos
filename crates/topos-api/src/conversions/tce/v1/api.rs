use prost::{bytes::Bytes, Message};

use super::v1::{
    watch_certificates_request::{Command, OpenStream},
    watch_certificates_response::{CertificatePushed, Event, StreamOpened},
    CheckpointRequest, CheckpointResponse, WatchCertificatesRequest, WatchCertificatesResponse,
};

macro_rules! impl_command_conversion {
    ($type: ident) => {
        impl From<$type> for WatchCertificatesRequest {
            fn from(command: $type) -> Self {
                Self {
                    request_id: Some(uuid::Uuid::new_v4().as_u64_pair().into()),
                    command: Some(Command::$type(command)),
                }
            }
        }
    };
}

macro_rules! impl_event_conversion {
    ($type: ident) => {
        impl From<$type> for WatchCertificatesResponse {
            fn from(event: $type) -> Self {
                Self {
                    request_id: None,
                    event: Some(Event::$type(event)),
                }
            }
        }
    };
}

impl_command_conversion!(OpenStream);

impl_event_conversion!(StreamOpened);
impl_event_conversion!(CertificatePushed);

impl From<CheckpointRequest> for Vec<u8> {
    fn from(val: CheckpointRequest) -> Self {
        val.encode_to_vec()
    }
}

impl From<Vec<u8>> for CheckpointResponse {
    fn from(input: Vec<u8>) -> Self {
        let bytes = Bytes::from(input);
        prost::Message::decode(bytes).unwrap()
    }
}
