use super::v1::ValidatorId;
use topos_crypto::messages::H160;
use topos_crypto::validator_id::{Error, VALIDATOR_ID_LENGTH};

impl std::fmt::Display for ValidatorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(&self.value))
    }
}

impl From<topos_crypto::validator_id::ValidatorId> for ValidatorId {
    fn from(other: topos_crypto::validator_id::ValidatorId) -> Self {
        ValidatorId {
            value: other.as_bytes().to_vec(),
        }
    }
}

impl TryFrom<ValidatorId> for topos_crypto::validator_id::ValidatorId {
    type Error = Error;

    fn try_from(other: ValidatorId) -> Result<Self, Self::Error> {
        if other.value.len() != VALIDATOR_ID_LENGTH {
            return Err(Error::InvalidByteLength(hex::encode(other.value)));
        }
        let mut value = [0; VALIDATOR_ID_LENGTH];

        value.copy_from_slice(other.value.as_slice());

        Ok(H160::from_slice(&value).into())
    }
}

impl PartialEq<ValidatorId> for topos_crypto::validator_id::ValidatorId {
    fn eq(&self, other: &ValidatorId) -> bool {
        if other.value.len() != VALIDATOR_ID_LENGTH {
            return false;
        }
        self.as_bytes() == &other.value[..VALIDATOR_ID_LENGTH]
    }
}
