use topos_core::uci::{Certificate, CertificateId};

pub mod inmemory;

#[async_trait::async_trait]
pub trait Storage: Send + 'static {
    async fn persist(&mut self, certificate: Certificate) -> Result<(), StorageError>;
    async fn remove(&mut self, certificate_id: &CertificateId) -> bool;
}

pub enum StorageError {
    CertificateAlreadyExist,
}
