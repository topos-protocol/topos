use super::v1::Uuid;

impl From<(u64, u64)> for Uuid {
    fn from((most_significant_bits, least_significant_bits): (u64, u64)) -> Self {
        Self {
            most_significant_bits,
            least_significant_bits,
        }
    }
}

impl From<Uuid> for uuid::Uuid {
    fn from(proto: Uuid) -> Self {
        Self::from_u64_pair(proto.most_significant_bits, proto.least_significant_bits)
    }
}

impl From<uuid::Uuid> for Uuid {
    fn from(uuid: uuid::Uuid) -> Self {
        uuid.as_u64_pair().into()
    }
}
