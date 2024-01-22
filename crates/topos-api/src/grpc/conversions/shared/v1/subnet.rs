use topos_core::uci::SUBNET_ID_LENGTH;

use super::v1::SubnetId;
use base64ct::{Base64, Encoding};

impl std::fmt::Display for SubnetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Base64::encode_string(&self.value))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to parse subnetId ({0})")]
    ValidationError(SubnetId),

    #[error("Unable to parse UCI field ({0}))")]
    UCI(#[from] topos_core::uci::Error),

    #[error("Missing mandatory field: {0}")]
    MissingField(&'static str),

    #[error("Invalid or missing state_root")]
    InvalidStateRoot,

    #[error("Invalid or missing tx_root_hash")]
    InvalidTxRootHash,

    #[error("Invalid or missing receipts_root_hash")]
    InvalidReceiptsRootHash,
}

impl From<[u8; SUBNET_ID_LENGTH]> for SubnetId {
    fn from(value: [u8; SUBNET_ID_LENGTH]) -> Self {
        SubnetId {
            value: value.to_vec(),
        }
    }
}

impl TryFrom<SubnetId> for [u8; SUBNET_ID_LENGTH] {
    type Error = Error;

    fn try_from(value: SubnetId) -> Result<Self, Self::Error> {
        if value.value.len() != SUBNET_ID_LENGTH {
            return Err(Error::ValidationError(value));
        }
        let mut id = [0; SUBNET_ID_LENGTH];

        id.copy_from_slice(value.value.as_slice());

        Ok(id)
    }
}

impl From<topos_core::uci::SubnetId> for SubnetId {
    fn from(value: topos_core::uci::SubnetId) -> Self {
        SubnetId {
            value: value.as_array().to_vec(),
        }
    }
}

impl TryFrom<SubnetId> for topos_core::uci::SubnetId {
    type Error = Error;

    fn try_from(value: SubnetId) -> Result<Self, Self::Error> {
        if value.value.len() != SUBNET_ID_LENGTH {
            return Err(Error::ValidationError(value));
        }
        let mut id = [0; SUBNET_ID_LENGTH];

        id.copy_from_slice(value.value.as_slice());

        Ok(id.into())
    }
}
