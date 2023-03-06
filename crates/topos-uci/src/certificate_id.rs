use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::Error;

#[derive(Serialize, Hash, Deserialize, Default, PartialEq, Eq, Clone, Copy)]
pub struct CertificateId {
    id: [u8; 32],
}

impl Display for CertificateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.id))
    }
}

impl Debug for CertificateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.id))
    }
}

impl Ord for CertificateId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for CertificateId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl From<[u8; 32]> for CertificateId {
    fn from(value: [u8; 32]) -> Self {
        Self { id: value }
    }
}

impl From<CertificateId> for Vec<u8> {
    fn from(value: CertificateId) -> Vec<u8> {
        value.id.to_vec()
    }
}

impl TryFrom<Vec<u8>> for CertificateId {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.len() != 32 {
            return Err(Error::ValidationError);
        }
        let mut id = [0; 32];

        id.copy_from_slice(value.as_slice());

        Ok(Self { id })
    }
}

impl CertificateId {
    pub const fn from_array(id: [u8; 32]) -> Self {
        Self { id }
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.id
    }
}
