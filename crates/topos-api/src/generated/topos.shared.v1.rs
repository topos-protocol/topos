#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Uuid {
    #[prost(uint64, tag="1")]
    pub most_significant_bits: u64,
    #[prost(uint64, tag="2")]
    pub least_significant_bits: u64,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubnetId {
    #[prost(string, tag="1")]
    pub value: ::prost::alloc::string::String,
}
