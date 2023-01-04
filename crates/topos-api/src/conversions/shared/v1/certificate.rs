use super::v1::CertificateId;

impl From<CertificateId> for String {
    fn from(value: CertificateId) -> Self {
        hex::encode(value.value)
    }
}

impl From<[u8; 32]> for CertificateId {
    fn from(value: [u8; 32]) -> Self {
        CertificateId {
            value: value.to_vec(),
        }
    }
}
