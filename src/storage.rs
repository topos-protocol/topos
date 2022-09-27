use thiserror::Error;
use topos_core::uci::{Certificate, CertificateId};

pub mod inmemory;

#[async_trait::async_trait]
pub trait Storage: Send + 'static {
    async fn persist(&mut self, certificate: Certificate) -> Result<(), StorageError>;
    async fn remove(&mut self, certificate_id: &CertificateId) -> bool;
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("The certificate already exists")]
    CertificateAlreadyExists,
}
