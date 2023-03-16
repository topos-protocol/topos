use super::v1::CertificateId;

impl std::fmt::Display for CertificateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.value))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to parse certificateId ({0})")]
    ValidationError(CertificateId),
}

impl From<[u8; 32]> for CertificateId {
    fn from(value: [u8; 32]) -> Self {
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
        if value.value.len() != 32 {
            return Err(Error::ValidationError(value));
        }
        let mut id = [0; 32];

        id.copy_from_slice(value.value.as_slice());

        Ok(id.into())
    }
}

impl PartialEq<CertificateId> for topos_uci::CertificateId {
    fn eq(&self, other: &CertificateId) -> bool {
        if other.value.len() != 32 {
            return false;
        }
        self.as_array() == &other.value[..32]
    }
}
