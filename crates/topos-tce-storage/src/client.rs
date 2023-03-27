use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId, SubnetId, CERTIFICATE_ID_LENGTH};

use crate::command::GetPendingCertificates;
use crate::{
    command::{
        AddPendingCertificate, CertificateDelivered, CheckPendingCertificateExists,
        FetchCertificates, GetCertificate, GetSourceHead, RemovePendingCertificate, StorageCommand,
        TargetedBy,
    },
    errors::StorageError,
    CertificatePositions, FetchCertificatesFilter, FetchCertificatesPosition, PendingCertificateId,
};

#[derive(Debug, Clone)]
pub struct StorageClient {
    sender: mpsc::Sender<StorageCommand>,
    pub(crate) shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
}

impl StorageClient {
    /// Create a new StorageClient
    pub(crate) fn new(
        sender: mpsc::Sender<StorageCommand>,
        shutdown_channel: mpsc::Sender<oneshot::Sender<()>>,
    ) -> Self {
        Self {
            sender,
            shutdown_channel,
        }
    }

    pub async fn targeted_by(
        &self,
        target_subnet_id: SubnetId,
    ) -> Result<Vec<SubnetId>, StorageError> {
        TargetedBy { target_subnet_id }.send_to(&self.sender).await
    }

    pub async fn pending_certificate_exists(
        &self,
        certificate_id: CertificateId,
    ) -> Result<(PendingCertificateId, Certificate), StorageError> {
        CheckPendingCertificateExists { certificate_id }
            .send_to(&self.sender)
            .await
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

    /// Ask the storage to remove a pending certificate
    pub async fn remove_pending_certificate(
        &self,
        pending_certificate_id: PendingCertificateId,
    ) -> Result<PendingCertificateId, StorageError> {
        RemovePendingCertificate {
            pending_certificate_id,
        }
        .send_to(&self.sender)
        .await
    }

    /// Fetch all pending certificates
    ///
    /// Return list of pending certificates
    pub async fn get_pending_certificates(
        &self,
        subnet_ids: Vec<SubnetId>,
    ) -> Result<Vec<Certificate>, StorageError> {
        GetPendingCertificates { subnet_ids }
            .send_to(&self.sender)
            .await
    }

    /// Ask the storage to tag this certificate as delivered.
    pub async fn certificate_delivered(
        &self,
        certificate_id: CertificateId,
    ) -> Result<CertificatePositions, StorageError> {
        CertificateDelivered { certificate_id }
            .send_to(&self.sender)
            .await
    }

    pub async fn fetch_certificates(
        &self,
        filter: FetchCertificatesFilter,
    ) -> Result<Vec<(Certificate, FetchCertificatesPosition)>, StorageError> {
        FetchCertificates { filter }.send_to(&self.sender).await
    }

    /// Fetch source head certificate for subnet
    ///
    /// Return position of the certificate and certificate itself
    pub async fn get_source_head(
        &self,
        subnet_id: SubnetId,
    ) -> Result<(u64, Certificate), StorageError> {
        GetSourceHead { subnet_id }.send_to(&self.sender).await
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
        if cert.prev_id.as_array() == &[0u8; CERTIFICATE_ID_LENGTH] {
            return Ok(());
        }

        self.get_certificate(cert.prev_id).await.map(|_| Ok(()))?
    }

    pub async fn shutdown(&self) -> Result<(), StorageError> {
        let (sender, receiver) = oneshot::channel();
        self.shutdown_channel
            .send(sender)
            .await
            .map_err(StorageError::ShutdownCommunication)?;

        Ok(receiver.await?)
    }
}
