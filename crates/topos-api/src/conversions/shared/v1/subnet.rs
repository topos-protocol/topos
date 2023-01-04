use super::v1::SubnetId;

impl From<SubnetId> for String {
    fn from(value: SubnetId) -> Self {
        hex::encode(value.value)
    }
}

impl From<[u8; 32]> for SubnetId {
    fn from(value: [u8; 32]) -> Self {
        SubnetId {
            value: value.to_vec(),
        }
    }
}
