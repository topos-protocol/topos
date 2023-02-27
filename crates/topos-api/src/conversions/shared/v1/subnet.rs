use super::v1::SubnetId;

impl std::fmt::Display for SubnetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.value))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to parse subnetId ({0})")]
    ValidationError(SubnetId),
}

impl From<[u8; 32]> for SubnetId {
    fn from(value: [u8; 32]) -> Self {
        SubnetId {
            value: value.to_vec(),
        }
    }
}

impl TryFrom<SubnetId> for [u8; 32] {
    type Error = Error;

    fn try_from(value: SubnetId) -> Result<Self, Self::Error> {
        if value.value.len() != 32 {
            return Err(Error::ValidationError(value));
        }
        let mut id = [0; 32];

        id.copy_from_slice(value.value.as_slice());

        Ok(id)
    }
}

impl From<topos_uci::SubnetId> for SubnetId {
    fn from(value: topos_uci::SubnetId) -> Self {
        SubnetId {
            value: value.as_array().to_vec(),
        }
    }
}

impl TryFrom<SubnetId> for topos_uci::SubnetId {
    type Error = Error;

    fn try_from(value: SubnetId) -> Result<Self, Self::Error> {
        if value.value.len() != 32 {
            return Err(Error::ValidationError(value));
        }
        let mut id = [0; 32];

        id.copy_from_slice(value.value.as_slice());

        Ok(id.into())
    }
}
