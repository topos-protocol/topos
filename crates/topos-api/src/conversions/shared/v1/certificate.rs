use super::v1::CertificateId;

impl std::fmt::Display for CertificateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(&self.value))
    }
}

impl From<[u8; 32]> for CertificateId {
    fn from(value: [u8; 32]) -> Self {
        CertificateId {
            value: value.to_vec(),
        }
    }
}
