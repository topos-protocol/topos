use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::str::FromStr;

use super::{Error, SUBNET_ID_LENGTH};

#[derive(Serialize, Hash, Deserialize, Default, PartialEq, Eq, Clone, Copy)]
pub struct SubnetId {
    id: [u8; SUBNET_ID_LENGTH],
}

impl Display for SubnetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.id))
    }
}

impl Debug for SubnetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.id))
    }
}

impl Ord for SubnetId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for SubnetId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl From<[u8; SUBNET_ID_LENGTH]> for SubnetId {
    fn from(value: [u8; SUBNET_ID_LENGTH]) -> Self {
        Self { id: value }
    }
}

impl From<SubnetId> for [u8; SUBNET_ID_LENGTH] {
    fn from(value: SubnetId) -> Self {
        value.id
    }
}

impl From<SubnetId> for Vec<u8> {
    fn from(value: SubnetId) -> Vec<u8> {
        value.id.to_vec()
    }
}

impl TryFrom<&[u8]> for SubnetId {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() != SUBNET_ID_LENGTH {
            return Err(Error::ValidationError(format!(
                "invalid subnet id of length {}, expected length {SUBNET_ID_LENGTH}",
                value.len()
            )));
        }

        let mut id = [0; SUBNET_ID_LENGTH];
        id.copy_from_slice(value);

        Ok(Self { id })
    }
}

impl FromStr for SubnetId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = if s.starts_with("0x") {
            hex::decode(&s[2..s.len()]).map_err(|e| {
                Error::ValidationError(format!(
                    "could not decode subnet id hex encoded string '{s}' error: {e}"
                ))
            })?
        } else {
            s.as_bytes().to_vec()
        };

        s.as_slice().try_into()
    }
}

impl SubnetId {
    pub const fn from_array(id: [u8; SUBNET_ID_LENGTH]) -> Self {
        Self { id }
    }

    pub const fn as_array(&self) -> &[u8; SUBNET_ID_LENGTH] {
        &self.id
    }

    pub fn to_secp256k1_public_key(&self) -> [u8; 33] {
        let mut public_key: [u8; 33] = [0; 33];
        public_key[0] = 0x02;
        public_key[1..(self.id.len() + 1)].copy_from_slice(&self.id[..]);
        public_key
    }
}
