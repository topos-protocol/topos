use self::checkpoints::StreamPositionError;

use tonic::transport::Channel;

use self::tce::v1::synchronizer_service_client::SynchronizerServiceClient;

pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("generated/topos.bin");

pub mod checkpoints;

pub trait GrpcClient {
    type Output;

    fn init(destination: Channel) -> Self::Output;
}

impl GrpcClient for SynchronizerServiceClient<Channel> {
    type Output = Self;

    fn init(channel: Channel) -> Self::Output {
        SynchronizerServiceClient::new(channel)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ConversionError {
    #[error(transparent)]
    GrpcDecode(#[from] prost::DecodeError),

    #[error("Missing mandatory field: {0}")]
    MissingField(&'static str),

    #[error(transparent)]
    StreamConversion(#[from] StreamPositionError),
}

#[path = ""]
pub mod tce {
    #[rustfmt::skip]
    #[allow(warnings)]
    #[path = "generated/topos.tce.v1.rs"]
    pub mod v1;

    #[path = "conversions/tce/v1/mod.rs"]
    pub mod v1_conversions;
}

#[path = ""]
pub mod shared {
    #[rustfmt::skip]
    #[allow(warnings)]
    #[path = "generated/topos.shared.v1.rs"]
    pub mod v1;

    #[path = "conversions/shared/v1/uuid.rs"]
    pub mod v1_conversions_uuid;

    #[path = "conversions/shared/v1/subnet.rs"]
    pub mod v1_conversions_subnet;

    #[path = "conversions/shared/v1/certificate.rs"]
    pub mod v1_conversions_certificate;
}

#[path = "."]
pub mod uci {
    #[rustfmt::skip]
    #[allow(warnings)]
    #[path = "generated/topos.uci.v1.rs"]
    pub mod v1;

    #[path = "conversions/uci/v1/uci.rs"]
    pub mod v1_conversions;
}
