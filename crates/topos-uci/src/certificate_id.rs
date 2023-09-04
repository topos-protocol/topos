use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;

use crate::{Error, CERTIFICATE_ID_LENGTH};

#[derive(Serialize, Hash, Deserialize, Default, PartialEq, Eq, Clone, Copy)]
pub struct CertificateId {
    id: [u8; CERTIFICATE_ID_LENGTH],
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

impl From<[u8; CERTIFICATE_ID_LENGTH]> for CertificateId {
    fn from(value: [u8; CERTIFICATE_ID_LENGTH]) -> Self {
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
        if value.len() != CERTIFICATE_ID_LENGTH {
            return Err(Error::ValidationError);
        }
        let mut id = [0; CERTIFICATE_ID_LENGTH];

        id.copy_from_slice(value.as_slice());

        Ok(Self { id })
    }
}

impl From<String> for CertificateId {
    fn from(input: String) -> Self {
        let id = if input.starts_with("0x") {
            hex::decode(&input[2..]).unwrap_or_else(|_| vec![0u8; CERTIFICATE_ID_LENGTH])
        } else {
            hex::decode(&input).unwrap_or_else(|_| vec![0u8; CERTIFICATE_ID_LENGTH])
        };

        let id_array: [u8; CERTIFICATE_ID_LENGTH] = id
            .as_slice()
            .try_into()
            .unwrap_or_else(|_| [0u8; CERTIFICATE_ID_LENGTH]);

        CertificateId { id: id_array }
    }
}

impl CertificateId {
    pub const fn from_array(id: [u8; CERTIFICATE_ID_LENGTH]) -> Self {
        Self { id }
    }

    pub const fn as_array(&self) -> &[u8; CERTIFICATE_ID_LENGTH] {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CERTIFICATE_ID_WITH_PREFIX: &str =
        "0x11db8713a79c41625f4bb2221bd43ac4766fff23e78f82212f48713a6768e76a";
    const CERTIFICATE_ID_WITHOUT_PREFIX: &str =
        "11db8713a79c41625f4bb2221bd43ac4766fff23e78f82212f48713a6768e76a";
    const MALFORMATTED_CERTIFICATE_ID: &str = "invalid_hex_string";

    #[test]
    fn convert_cert_id_string_with_prefix() {
        let certificate_id1: CertificateId = CERTIFICATE_ID_WITH_PREFIX.to_string().into();

        assert_eq!(
            &certificate_id1.id[..],
            &[
                0x11, 0xdb, 0x87, 0x13, 0xa7, 0x9c, 0x41, 0x62, 0x5f, 0x4b, 0xb2, 0x22, 0x1b, 0xd4,
                0x3a, 0xc4, 0x76, 0x6f, 0xff, 0x23, 0xe7, 0x8f, 0x82, 0x21, 0x2f, 0x48, 0x71, 0x3a,
                0x67, 0x68, 0xe7, 0x6a
            ][..]
        )
    }

    #[test]
    fn convert_cert_id_string_without_prefix() {
        let certificate_id1: CertificateId = CERTIFICATE_ID_WITHOUT_PREFIX.to_string().into();

        assert_eq!(
            &certificate_id1.id[..],
            &[
                0x11, 0xdb, 0x87, 0x13, 0xa7, 0x9c, 0x41, 0x62, 0x5f, 0x4b, 0xb2, 0x22, 0x1b, 0xd4,
                0x3a, 0xc4, 0x76, 0x6f, 0xff, 0x23, 0xe7, 0x8f, 0x82, 0x21, 0x2f, 0x48, 0x71, 0x3a,
                0x67, 0x68, 0xe7, 0x6a
            ][..]
        )
    }

    #[test]
    fn malformatted_cert_id() {
        let certificate_id3: CertificateId = MALFORMATTED_CERTIFICATE_ID.to_string().into();

        assert_eq!(
            &certificate_id3.id[..],
            &[
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00,
            ][..]
        )
    }
}
