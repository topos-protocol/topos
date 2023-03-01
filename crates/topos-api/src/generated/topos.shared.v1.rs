#[derive(Copy)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Uuid {
    #[prost(uint64, tag = "1")]
    pub most_significant_bits: u64,
    #[prost(uint64, tag = "2")]
    pub least_significant_bits: u64,
}
#[derive(Eq, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubnetId {
    #[prost(bytes = "vec", tag = "1")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Eq, Hash)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CertificateId {
    #[prost(bytes = "vec", tag = "1")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
/// Checkpoints are used to walk through streams
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Checkpoints {}
/// Nested message and enum types in `Checkpoints`.
pub mod checkpoints {
    /// SourceCheckpoint represents a snapshot of multiple stream's positions regarding
    /// one or multiple source subnets.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct SourceCheckpoint {
        #[prost(message, repeated, tag = "1")]
        pub source_subnet_ids: ::prost::alloc::vec::Vec<super::SubnetId>,
        #[prost(message, repeated, tag = "2")]
        pub positions: ::prost::alloc::vec::Vec<super::positions::SourceStreamPosition>,
    }
    /// TargetCheckpoint represents a snapshot of multiple stream's positions regarding
    /// one or multiple target subnets.
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TargetCheckpoint {
        #[prost(message, repeated, tag = "1")]
        pub target_subnet_ids: ::prost::alloc::vec::Vec<super::SubnetId>,
        #[prost(message, repeated, tag = "2")]
        pub positions: ::prost::alloc::vec::Vec<super::positions::TargetStreamPosition>,
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Positions {}
/// Nested message and enum types in `Positions`.
pub mod positions {
    /// SourceStreamPosition represents a single point in a source stream.
    /// It is defined by a source_subnet_id and a position, resolving to a certificate_id
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct SourceStreamPosition {
        /// The source_subnet_id is a mandatory field for the SourceStreamPosition
        #[prost(message, optional, tag = "1")]
        pub source_subnet_id: ::core::option::Option<super::SubnetId>,
        #[prost(uint64, tag = "2")]
        pub position: u64,
        #[prost(message, optional, tag = "3")]
        pub certificate_id: ::core::option::Option<super::CertificateId>,
    }
    /// TargetStreamPosition represents a single point in a target stream regarding a source subnet.
    /// It is defined by a target_subnet_id, source_subnet_id and a position, resolving to a certificate_id
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct TargetStreamPosition {
        /// The source_subnet_id is a mandatory field for the TargetStreamPosition
        #[prost(message, optional, tag = "1")]
        pub source_subnet_id: ::core::option::Option<super::SubnetId>,
        /// The target_subnet_id is a mandatory field for the TargetStreamPosition
        #[prost(message, optional, tag = "2")]
        pub target_subnet_id: ::core::option::Option<super::SubnetId>,
        #[prost(uint64, tag = "3")]
        pub position: u64,
        #[prost(message, optional, tag = "4")]
        pub certificate_id: ::core::option::Option<super::CertificateId>,
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Frost {
    #[prost(bytes = "vec", tag = "1")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StarkProof {
    #[prost(bytes = "vec", tag = "1")]
    pub value: ::prost::alloc::vec::Vec<u8>,
}
