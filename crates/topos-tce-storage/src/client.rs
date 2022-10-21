use tokio::sync::mpsc;
use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{
    command::{AddPendingCertificate, CertificateDelivered, GetCertificate, StorageCommand},
    errors::StorageError,
    PendingCertificateId,
};

#[derive(Debug, Clone)]
pub struct StorageClient {
    sender: mpsc::Sender<StorageCommand>,
}

impl StorageClient {
    /// Create a new StorageClient
    #[allow(dead_code)]
    pub(crate) fn new(sender: mpsc::Sender<StorageCommand>) -> Self {
        Self { sender }
    }

    /// Ask the storage to add a new pending certificate
    ///
    /// The storage will not check if the certificate is already existing.
    pub async fn add_pending_certificate(
        &self,
        certificate: Certificate,
    ) -> Result<PendingCertificateId, StorageError> {
        AddPendingCertificate { certificate }
            .send_to(&self.sender)
            .await
    }

    /// Ask the storage to tag this certificate as delivered.
    pub async fn certificate_delivered(
        &self,
        certificate_id: CertificateId,
    ) -> Result<(), StorageError> {
        CertificateDelivered { certificate_id }
            .send_to(&self.sender)
            .await
    }

    /// Retrieves the certificates delivered to one subnet
    pub async fn recent_certificates_for_subnet(
        &self,
        _subnet_id: SubnetId,
        _limit: u64,
    ) -> Result<Vec<Certificate>, StorageError> {
        unimplemented!()
    }

    /// Fetch a certificate from the storage
    ///
    /// Only delivered certificates are returned
    pub async fn get_certificate(
        &self,
        certificate_id: CertificateId,
    ) -> Result<Certificate, StorageError> {
        GetCertificate { certificate_id }
            .send_to(&self.sender)
            .await
    }

    /// Used to see if a certificate is ready to be delivered
    ///
    /// TODO: Will be removed to use the queue of the connection
    pub async fn check_precedence(&self, cert: &Certificate) -> Result<(), StorageError> {
        if cert.prev_cert_id == "0" {
            return Ok(());
        }

        self.get_certificate(cert.prev_cert_id.clone())
            .await
            .map(|_| Ok(()))?
    }
}
