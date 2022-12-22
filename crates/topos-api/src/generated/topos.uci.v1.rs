/// Two types of transactions available:
/// Asset transfer
/// Smart contract function call
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CrossChainTransactionData {
    #[prost(oneof = "cross_chain_transaction_data::Data", tags = "1, 2, 3")]
    pub data: ::core::option::Option<cross_chain_transaction_data::Data>,
}
/// Nested message and enum types in `CrossChainTransactionData`.
pub mod cross_chain_transaction_data {
    /// Cross chain transaction data for asset transfer
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct AssetTransfer {
        #[prost(bytes = "vec", tag = "1")]
        pub sender: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "2")]
        pub receiver: ::prost::alloc::vec::Vec<u8>,
        #[prost(string, tag = "3")]
        pub symbol: ::prost::alloc::string::String,
        #[prost(bytes = "vec", tag = "4")]
        pub amount: ::prost::alloc::vec::Vec<u8>,
    }
    /// Data describing cross chain function call
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct ContractCall {
        #[prost(bytes = "vec", tag = "1")]
        pub source_contract_addr: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "2")]
        pub target_contract_addr: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "3")]
        pub payload_hash: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "4")]
        pub payload: ::prost::alloc::vec::Vec<u8>,
    }
    /// Data describing cross chain function call with token transfer
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct ContractCallWithToken {
        #[prost(bytes = "vec", tag = "1")]
        pub source_contract_addr: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "2")]
        pub target_contract_addr: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "3")]
        pub payload_hash: ::prost::alloc::vec::Vec<u8>,
        #[prost(bytes = "vec", tag = "4")]
        pub payload: ::prost::alloc::vec::Vec<u8>,
        #[prost(string, tag = "5")]
        pub symbol: ::prost::alloc::string::String,
        #[prost(bytes = "vec", tag = "6")]
        pub amount: ::prost::alloc::vec::Vec<u8>,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Data {
        #[prost(message, tag = "1")]
        AssetTransfer(AssetTransfer),
        #[prost(message, tag = "2")]
        ContractCall(ContractCall),
        #[prost(message, tag = "3")]
        ContractCallWithToken(ContractCallWithToken),
    }
}
/// Cross chain transaction exchanged between subnets
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CrossChainTransaction {
    #[prost(message, optional, tag = "1")]
    pub target_subnet_id: ::core::option::Option<super::super::shared::v1::SubnetId>,
    #[prost(message, optional, tag = "2")]
    pub transaction_data: ::core::option::Option<CrossChainTransactionData>,
}
/// Certificate - main exchange item
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Certificate {
    #[prost(message, optional, tag = "1")]
    pub source_subnet_id: ::core::option::Option<super::super::shared::v1::SubnetId>,
    #[prost(message, optional, tag = "2")]
    pub id: ::core::option::Option<super::super::shared::v1::CertificateId>,
    #[prost(message, optional, tag = "3")]
    pub prev_id: ::core::option::Option<super::super::shared::v1::CertificateId>,
    #[prost(message, repeated, tag = "4")]
    pub calls: ::prost::alloc::vec::Vec<CrossChainTransaction>,
}
