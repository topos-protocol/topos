use async_trait::async_trait;
use topos_core::uci::Certificate;

use crate::{
    command::{
        AddPendingCertificate, CertificateDelivered, FetchCertificates, GetCertificate,
        RemovePendingCertificate,
    },
    errors::StorageError,
    Connection, FetchCertificatesFilter, Height, PendingCertificateId, Storage,
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

/// Handle a RemovePendingCertificate query
///
/// a RemovePendingCertificate query will try to remove the pending certificate from the storage
/// and remove the pending_id from the pending_certificates queue
#[async_trait]
impl<S> CommandHandler<RemovePendingCertificate> for Connection<S>
where
    S: Storage,
{
    async fn handle(
        &mut self,
        RemovePendingCertificate {
            pending_certificate_id,
        }: RemovePendingCertificate,
    ) -> Result<PendingCertificateId, StorageError> {
        _ = self
            .storage
            .remove_pending_certificate(pending_certificate_id)
            .await?;

        if let Some(index) = self
            .pending_certificates
            .iter()
            .position(|p| *p == pending_certificate_id)
        {
            self.pending_certificates.remove(index);
        }
        Ok(pending_certificate_id)
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

/// Handle a FetchCertificates query
///
/// The FetchCertificates query will fetch certificates from the storage based on the provided
/// filter
#[async_trait]
impl<S> CommandHandler<FetchCertificates> for Connection<S>
where
    S: Storage,
{
    async fn handle(
        &mut self,
        FetchCertificates { filter }: FetchCertificates,
    ) -> Result<Vec<Certificate>, StorageError> {
        let certificate_ids = match filter {
            FetchCertificatesFilter::Source {
                subnet_id,
                version,
                limit,
            } => {
                self.storage
                    .get_certificates_by_source(subnet_id, Height(version), limit)
                    .await?
            }
            FetchCertificatesFilter::Target {
                target_subnet_id,
                source_subnet_id,
                version,
                limit,
            } => {
                self.storage
                    .get_certificates_by_target(
                        target_subnet_id,
                        source_subnet_id,
                        Height(version),
                        limit,
                    )
                    .await?
            }
        };

        Ok(self.storage.get_certificates(certificate_ids).await?)
    }
}
