use std::collections::BTreeMap;

use topos_core::uci::{Certificate, CertificateId};

use super::{Storage, StorageError};

#[derive(Default)]
pub struct InmemoryStorage {
    pool: BTreeMap<CertificateId, Certificate>,
}

#[async_trait::async_trait]
impl Storage for InmemoryStorage {
    async fn persist(&mut self, certificate: Certificate) -> Result<(), StorageError> {
        if self
            .pool
            .insert(certificate.cert_id.clone(), certificate)
            .is_none()
        {
            Ok(())
        } else {
            Err(StorageError::CertificateAlreadyExists)
        }
    }

    async fn remove(&mut self, certificate_id: &CertificateId) -> bool {
        self.pool.remove(certificate_id).is_some()
    }
}
