use async_trait::async_trait;
use topos_core::uci::Certificate;

use crate::{
    command::{AddPendingCertificate, CertificateDelivered, GetCertificate},
    errors::StorageError,
    Connection, PendingCertificateId, Storage,
};

use super::handler::CommandHandler;

/// Handle a AddPendingCertificate query
///
/// a AddPendingCertificate query will put the pending certificate into the storage
/// and add the returned pending_id into the pending_certificates queue
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

/// Handle a CertificateDelivered query
#[async_trait]
impl<S> CommandHandler<CertificateDelivered> for Connection<S>
where
    S: Storage,
{
    async fn handle(&mut self, _command: CertificateDelivered) -> Result<(), StorageError> {
        Ok(())
    }
}

/// Handle a GetCertificate query
///
/// The GetCertificate query will just ask for delivered certificate
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
