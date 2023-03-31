use topos_uci::CERTIFICATE_ID_LENGTH;

use super::v1::CertificateId;
use base64::Engine;

impl std::fmt::Display for CertificateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            base64::engine::general_purpose::STANDARD.encode(&self.value)
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to parse certificateId ({0})")]
    ValidationError(CertificateId),
}

impl From<[u8; CERTIFICATE_ID_LENGTH]> for CertificateId {
    fn from(value: [u8; CERTIFICATE_ID_LENGTH]) -> Self {
        CertificateId {
            value: value.to_vec(),
        }
    }
}

impl From<topos_uci::CertificateId> for CertificateId {
    fn from(value: topos_uci::CertificateId) -> Self {
        CertificateId {
            value: value.as_array().to_vec(),
        }
    }
}

impl TryFrom<CertificateId> for topos_uci::CertificateId {
    type Error = Error;

    fn try_from(value: CertificateId) -> Result<Self, Self::Error> {
        if value.value.len() != CERTIFICATE_ID_LENGTH {
            return Err(Error::ValidationError(value));
        }
        let mut id = [0; CERTIFICATE_ID_LENGTH];

        id.copy_from_slice(value.value.as_slice());

        Ok(id.into())
    }
}

impl PartialEq<CertificateId> for topos_uci::CertificateId {
    fn eq(&self, other: &CertificateId) -> bool {
        if other.value.len() != CERTIFICATE_ID_LENGTH {
            return false;
        }
        self.as_array() == &other.value[..CERTIFICATE_ID_LENGTH]
    }
}
