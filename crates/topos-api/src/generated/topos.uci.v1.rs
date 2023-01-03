/// Two types of transactions available:
/// Asset transfer
/// Smart contract function call
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CrossChainTransactionData {
    #[prost(oneof = "cross_chain_transaction_data::Data", tags = "1, 2")]
    pub data: ::core::option::Option<cross_chain_transaction_data::Data>,
}
/// Nested message and enum types in `CrossChainTransactionData`.
pub mod cross_chain_transaction_data {
    /// Cross chain transaction data for asset transfer
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct AssetTransfer {
        #[prost(string, tag = "1")]
        pub asset_id: ::prost::alloc::string::String,
        #[prost(bytes = "vec", tag = "2")]
        pub amount: ::prost::alloc::vec::Vec<u8>,
    }
    /// Data describing cross chain function call
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct FunctionCall {
        #[prost(bytes = "vec", tag = "1")]
        pub data: ::prost::alloc::vec::Vec<u8>,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Data {
        #[prost(message, tag = "1")]
        AssetTransfer(AssetTransfer),
        #[prost(message, tag = "2")]
        FunctionCall(FunctionCall),
    }
}
/// Cross chain transaction exchanged between subnets
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CrossChainTransaction {
    #[prost(string, tag = "1")]
    pub target_subnet_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub sender_addr: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub recipient_addr: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "4")]
    pub transaction_data: ::core::option::Option<CrossChainTransactionData>,
}
/// Certificate - main exchange item
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Certificate {
    #[prost(string, tag = "1")]
    pub source_subnet_id: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub cert_id: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub prev_cert_id: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "4")]
    pub calls: ::prost::alloc::vec::Vec<CrossChainTransaction>,
}
