use super::v1::SubnetId;

impl std::fmt::Display for SubnetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.value))
    }
}

impl From<[u8; 32]> for SubnetId {
    fn from(value: [u8; 32]) -> Self {
        SubnetId {
            value: value.to_vec(),
        }
    }
}
