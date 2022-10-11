use tokio::sync::{
    mpsc::{self},
    oneshot,
};
use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{
    command::{ExpectedVersion, StorageCommand},
    errors::StorageError,
    CertificateStatus,
};

#[derive(Debug, Clone)]
pub struct StorageClient {
    pub(crate) sender: mpsc::Sender<StorageCommand>,
}

impl StorageClient {
    pub async fn persist_pending(&self, certificate: Certificate) -> Result<(), StorageError> {
        let (response_channel, receiver) = oneshot::channel();

        self.sender
            .send(StorageCommand::Persist {
                certificate,
                status: CertificateStatus::Pending,
                response_channel,
            })
            .await?;

        receiver.await?
    }

    pub async fn certificate_delivered(
        &self,
        certificate_id: CertificateId,
    ) -> Result<(), StorageError> {
        let (response_channel, receiver) = oneshot::channel();

        self.sender
            .send(StorageCommand::UpdateCertificate {
                certificate_id,
                status: CertificateStatus::Delivered,
                response_channel,
            })
            .await?;

        receiver.await?
    }

    pub async fn recent_certificates_for_subnet(
        &self,
        subnet_id: SubnetId,
        limit: u64,
    ) -> Result<Vec<Certificate>, StorageError> {
        let (response_channel, receiver) = oneshot::channel();

        self.sender
            .send(StorageCommand::ReadStream {
                subnet_id,
                from: ExpectedVersion::End,
                limit,
                response_channel,
            })
            .await?;

        receiver.await?
    }

    pub async fn get_certificate(
        &self,
        certificate_id: CertificateId,
    ) -> Result<Certificate, StorageError> {
        let (response_channel, receiver) = oneshot::channel();

        self.sender
            .send(StorageCommand::GetCertificate {
                certificate_id,
                response_channel,
            })
            .await?;

        receiver.await?
    }

    pub async fn check_precedence(&self, cert: &Certificate) -> Result<(), StorageError> {
        if cert.prev_cert_id == "0" {
            return Ok(());
        }

        self.get_certificate(cert.prev_cert_id.clone())
            .await
            .map(|_| Ok(()))?
    }
}
