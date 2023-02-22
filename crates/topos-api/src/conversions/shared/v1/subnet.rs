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

impl TryFrom<SubnetId> for [u8; 32] {
    type Error = <[u8; 32] as TryFrom<Vec<u8>>>::Error;

    fn try_from(value: SubnetId) -> Result<Self, Self::Error> {
        value.value.try_into()
    }
}
