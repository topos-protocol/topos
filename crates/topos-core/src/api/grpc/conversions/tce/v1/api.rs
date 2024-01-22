use crate::api::grpc::tce::v1::{
    watch_certificates_request::{Command, OpenStream},
    watch_certificates_response::{CertificatePushed, Event, StreamOpened},
    WatchCertificatesRequest, WatchCertificatesResponse,
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
