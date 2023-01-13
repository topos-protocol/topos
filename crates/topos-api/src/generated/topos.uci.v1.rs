/// Certificate - main exchange item
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Certificate {
    #[prost(message, optional, tag = "1")]
    pub prev_id: ::core::option::Option<super::super::shared::v1::CertificateId>,
    #[prost(message, optional, tag = "2")]
    pub source_subnet_id: ::core::option::Option<super::super::shared::v1::SubnetId>,
    #[prost(message, repeated, tag = "3")]
    pub target_subnets: ::prost::alloc::vec::Vec<super::super::shared::v1::SubnetId>,
    #[prost(message, optional, tag = "4")]
    pub id: ::core::option::Option<super::super::shared::v1::CertificateId>,
}
