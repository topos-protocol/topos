use super::v1::SubnetId;

impl From<SubnetId> for String {
    fn from(value: SubnetId) -> Self {
        hex::encode(value.value)
    }
}
