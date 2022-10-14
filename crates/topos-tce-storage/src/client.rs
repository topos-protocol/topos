use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{Certificate, CertificateId, SubnetId};

use crate::{
    command::{AddPendingCertificate, CertificateDelivered, GetCertificate, StorageCommand},
    errors::StorageError,
    PendingCertificateId,
};

#[derive(Debug, Clone)]
pub struct StorageClient {
    pub(crate) sender: mpsc::Sender<StorageCommand>,
}

impl StorageClient {
    pub async fn add_pending_certificate(
        &self,
        certificate: Certificate,
    ) -> Result<PendingCertificateId, StorageError> {
        let (response_channel, receiver) = oneshot::channel();

        self.sender
            .send(StorageCommand::AddPendingCertificate(
                AddPendingCertificate { certificate },
                response_channel,
            ))
            .await?;

        receiver.await?
    }

    pub async fn certificate_delivered(
        &self,
        certificate_id: CertificateId,
    ) -> Result<(), StorageError> {
        let (response_channel, receiver) = oneshot::channel();

        self.sender
            .send(StorageCommand::CertificateDelivered(
                CertificateDelivered { certificate_id },
                response_channel,
            ))
            .await?;

        receiver.await?
    }

    pub async fn recent_certificates_for_subnet(
        &self,
        _subnet_id: SubnetId,
        _limit: u64,
    ) -> Result<Vec<Certificate>, StorageError> {
        unimplemented!()
        // let (response_channel, receiver) = oneshot::channel();
        //
        // self.sender
        //     .send(StorageCommand::ReadStream {
        //         subnet_id,
        //         from: ExpectedVersion::End,
        //         limit,
        //         response_channel,
        //     })
        //     .await?;
        //
        // receiver.await?
    }

    pub async fn get_certificate(
        &self,
        certificate_id: CertificateId,
    ) -> Result<Certificate, StorageError> {
        let (response_channel, receiver) = oneshot::channel();

        self.sender
            .send(StorageCommand::GetCertificate(
                GetCertificate { certificate_id },
                response_channel,
            ))
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
