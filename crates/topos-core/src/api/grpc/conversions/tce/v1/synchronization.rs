use prost::{bytes::Bytes, Message};

use crate::api::grpc::tce::v1::{
    CheckpointRequest, CheckpointResponse, FetchCertificatesRequest, FetchCertificatesResponse,
};

use crate::api::grpc::ConversionError;

macro_rules! impl_to_vec_conversion {
    ($($type: ident),*) => {
        $(
            impl From<$type> for Vec<u8> {
                fn from(val: $type) -> Self {
                    val.encode_to_vec()
                }
            }
        )*
    };
}

macro_rules! impl_from_vec_conversion {
    ($($type: ident),*) => {
        $(
            impl TryFrom<Vec<u8>> for $type {
                type Error = ConversionError;
                fn try_from(input: Vec<u8>) -> Result<Self, Self::Error> {
                    let bytes = Bytes::from(input);

                    Ok(Self::decode(bytes)?)
                }
            }
        )*
    };
}

impl_to_vec_conversion!(
    CheckpointRequest,
    CheckpointResponse,
    FetchCertificatesRequest,
    FetchCertificatesResponse
);

impl_from_vec_conversion!(
    CheckpointResponse,
    CheckpointRequest,
    FetchCertificatesRequest,
    FetchCertificatesResponse
);
