use async_trait::async_trait;
use topos_commands::CommandHandler;
use topos_core::uci::{Certificate, SubnetId};
use tracing::{debug, error, info};

use crate::command::GetPendingCertificates;
use crate::{
    command::{
        AddPendingCertificate, CertificateDelivered, CheckPendingCertificateExists,
        FetchCertificates, GetCertificate, GetSourceHead, RemovePendingCertificate, TargetedBy,
    },
    errors::StorageError,
    CertificatePositions, CertificateSourceStreamPosition, CertificateTargetStreamPosition,
    Connection, FetchCertificatesFilter, FetchCertificatesPosition, InternalStorageError,
    PendingCertificateId, Position, Storage,
};

/// Handle a AddPendingCertificate query
///
/// a AddPendingCertificate query will put the pending certificate into the storage
/// and add the returned pending_id into the pending_certificates queue
#[async_trait]
impl<S> CommandHandler<AddPendingCertificate> for Connection<S>
where
    S: Storage,
{
    type Error = StorageError;

    async fn handle(
        &mut self,
        AddPendingCertificate { certificate }: AddPendingCertificate,
    ) -> Result<PendingCertificateId, StorageError> {
        let pending_id = self.storage.add_pending_certificate(&certificate).await?;

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
    type Error = StorageError;

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

/// Handle a GetPendingCertificates query
///
/// The GetPendingCertificates query will ask for pending certificate
/// for list of subnets ids
#[async_trait]
impl<S> CommandHandler<GetPendingCertificates> for Connection<S>
where
    S: Storage,
{
    type Error = StorageError;

    async fn handle(
        &mut self,
        GetPendingCertificates { subnet_ids }: GetPendingCertificates,
    ) -> Result<Vec<Certificate>, StorageError> {
        let pending_certificates = match self.storage.get_pending_certificates().await {
            Ok(pending_certificates) => pending_certificates,
            Err(e) => {
                error!("Failure on the storage to get pending certificates: {e}");
                return Err(e.into());
            }
        };

        let subnet_ids = subnet_ids
            .into_iter()
            .collect::<std::collections::HashSet<SubnetId>>();
        let pending_certificates: Vec<Certificate> = pending_certificates
            .into_iter()
            .map(|v| v.1)
            .filter(|cert| subnet_ids.contains(&cert.source_subnet_id))
            .collect();

        Ok(pending_certificates)
    }
}

/// Handle a CertificateDelivered query
#[async_trait]
impl<S> CommandHandler<CertificateDelivered> for Connection<S>
where
    S: Storage,
{
    type Error = StorageError;

    async fn handle(
        &mut self,
        command: CertificateDelivered,
    ) -> Result<CertificatePositions, StorageError> {
        let certificate_id = command.certificate_id;

        let (pending_certificate_id, certificate) =
            self.storage.get_pending_certificate(certificate_id).await?;

        Ok(self
            .storage
            .persist(&certificate, Some(pending_certificate_id))
            .await?)
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
    type Error = StorageError;

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
    type Error = StorageError;

    async fn handle(
        &mut self,
        FetchCertificates { filter }: FetchCertificates,
    ) -> Result<Vec<(Certificate, FetchCertificatesPosition)>, StorageError> {
        let mut result = Vec::new();
        match filter {
            FetchCertificatesFilter::Source {
                source_stream_position:
                    CertificateSourceStreamPosition {
                        source_subnet_id: subnet_id,
                        position,
                    },
                limit,
            } => {
                let certificate_ids = self
                    .storage
                    .get_certificates_by_source(subnet_id, position, limit)
                    .await?;
                let certificates = self.storage.get_certificates(certificate_ids).await?;

                for (index, cert) in certificates.into_iter().enumerate() {
                    result.push((
                        cert,
                        FetchCertificatesPosition::Source(CertificateSourceStreamPosition {
                            source_subnet_id: subnet_id,
                            position: Position(position.0 + index as u64),
                        }),
                    ));
                }
                Ok(result)
            }
            FetchCertificatesFilter::Target {
                target_stream_position:
                    CertificateTargetStreamPosition {
                        target_subnet_id,
                        source_subnet_id,
                        position,
                    },
                limit,
            } => {
                info!(
                    "Fetching {limit} certificates from the Position {:?}",
                    Position(position.0)
                );
                let certificate_ids = self
                    .storage
                    .get_certificates_by_target(target_subnet_id, source_subnet_id, position, limit)
                    .await?;
                let certificates = self.storage.get_certificates(certificate_ids).await?;
                for (index, cert) in certificates.into_iter().enumerate() {
                    result.push((
                        cert,
                        FetchCertificatesPosition::Target(CertificateTargetStreamPosition {
                            target_subnet_id,
                            source_subnet_id,
                            position: Position(position.0 + index as u64),
                        }),
                    ));
                }
                Ok(result)
            }
        }
    }
}

/// Handle a GetSourceHead query
///
/// The GetSourceHead query will ask for latest head certificate
/// for particular subnet
#[async_trait]
impl<S> CommandHandler<GetSourceHead> for Connection<S>
where
    S: Storage,
{
    type Error = StorageError;

    async fn handle(
        &mut self,
        GetSourceHead { subnet_id }: GetSourceHead,
    ) -> Result<(u64, Certificate), StorageError> {
        let heads = match self.storage.get_source_heads(vec![subnet_id]).await {
            Ok(heads) => heads,
            Err(e) => {
                error!("Failure on the storage to get the source head: {e}");
                return Err(e.into());
            }
        };
        let source_head = heads.last().ok_or(StorageError::InternalStorage(
            InternalStorageError::MissingHeadForSubnet(subnet_id),
        ))?;
        debug!(
            "Source head certificate for subnet id {subnet_id} is {}",
            source_head.cert_id
        );
        let certificate = match self.storage.get_certificate(source_head.cert_id).await {
            Ok(certificate) => certificate,
            Err(e) => {
                error!(
                    "Failure on the storage to get the source head Certificate {}",
                    source_head.cert_id
                );
                return Err(e.into());
            }
        };
        Ok((source_head.position.0, certificate))
    }
}

#[async_trait]
impl<S> CommandHandler<CheckPendingCertificateExists> for Connection<S>
where
    S: Storage,
{
    type Error = StorageError;

    async fn handle(
        &mut self,
        CheckPendingCertificateExists { certificate_id }: CheckPendingCertificateExists,
    ) -> Result<(PendingCertificateId, Certificate), StorageError> {
        Ok(self.storage.get_pending_certificate(certificate_id).await?)
    }
}

#[async_trait]
impl<S> CommandHandler<TargetedBy> for Connection<S>
where
    S: Storage,
{
    type Error = StorageError;

    async fn handle(
        &mut self,
        TargetedBy { target_subnet_id }: TargetedBy,
    ) -> Result<Vec<SubnetId>, StorageError> {
        Ok(self
            .storage
            .get_source_list_by_target(target_subnet_id)
            .await?)
    }
}
