use crate::messages::{Address, H160};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

pub const VALIDATOR_ID_LENGTH: usize = 20;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to parse address string as H160")]
    ParseError,
    #[error("Failed to convert byte array into H160: {0}")]
    InvalidByteLength(String),
}

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ValidatorId(H160);

impl ValidatorId {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn address(&self) -> Address {
        self.0
    }
}

impl From<H160> for ValidatorId {
    fn from(address: H160) -> Self {
        ValidatorId(address)
    }
}

impl FromStr for ValidatorId {
    type Err = Error;

    fn from_str(address: &str) -> Result<Self, Self::Err> {
        H160::from_str(address)
            .map_err(|_| Error::ParseError)
            .map(ValidatorId)
    }
}

impl std::fmt::Display for ValidatorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}
