use async_trait::async_trait;
use topos_core::uci::Certificate;

use crate::{
    command::{AddPendingCertificate, CertificateDelivered, GetCertificate},
    errors::StorageError,
    Connection, PendingCertificateId, Storage,
};

use super::CommandHandler;

#[async_trait]
impl<S> CommandHandler<AddPendingCertificate> for Connection<S>
where
    S: Storage,
{
    async fn handle(
        &mut self,
        AddPendingCertificate { certificate }: AddPendingCertificate,
    ) -> Result<PendingCertificateId, StorageError> {
        let pending_id = self.storage.add_pending_certificate(certificate).await?;

        self.pending_certificates.push_back(pending_id);

        Ok(pending_id)
    }
}

#[async_trait]
impl<S> CommandHandler<CertificateDelivered> for Connection<S>
where
    S: Storage,
{
    async fn handle(&mut self, _command: CertificateDelivered) -> Result<(), StorageError> {
        Ok(())
    }
}

#[async_trait]
impl<S> CommandHandler<GetCertificate> for Connection<S>
where
    S: Storage,
{
    async fn handle(
        &mut self,
        GetCertificate { certificate_id }: GetCertificate,
    ) -> Result<Certificate, StorageError> {
        Ok(self.storage.get_certificate(certificate_id).await?)
    }
}
