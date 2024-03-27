use std::sync::Arc;

use topos_core::types::stream::CertificateTargetStreamPosition;
use topos_core::types::CertificateDelivered;
use topos_core::uci::{Certificate, SubnetId};

use crate::store::ReadStore;
use crate::validator::ValidatorStore;
use crate::{
    errors::StorageError, FetchCertificatesFilter, FetchCertificatesPosition, PendingCertificateId,
};

#[derive(Clone)]
pub struct StorageClient {
    store: Arc<ValidatorStore>,
}

impl StorageClient {
    /// Create a new StorageClient
    pub fn new(store: Arc<ValidatorStore>) -> Self {
        Self { store }
    }

    /// Return the list of all source subnets that targeted the given target subnet
    pub async fn get_target_source_subnet_list(
        &self,
        target_subnet_id: SubnetId,
    ) -> Result<Vec<SubnetId>, StorageError> {
        self.store.get_target_source_subnet_list(&target_subnet_id)
    }

    /// Fetch all pending certificates
    ///
    /// Return list of pending certificates
    pub async fn get_pending_certificates(
        &self,
    ) -> Result<Vec<(PendingCertificateId, Certificate)>, StorageError> {
        Ok(self.store.iter_pending_pool()?.collect())
    }

    pub async fn fetch_certificates(
        &self,
        filter: FetchCertificatesFilter,
    ) -> Result<Vec<(CertificateDelivered, FetchCertificatesPosition)>, StorageError> {
        match filter {
            FetchCertificatesFilter::Source { .. } => unimplemented!(),
            FetchCertificatesFilter::Target {
                target_stream_position,
                limit,
            } => self
                .store
                .get_target_stream_certificates_from_position(
                    CertificateTargetStreamPosition::new(
                        target_stream_position.target_subnet_id,
                        target_stream_position.source_subnet_id,
                        target_stream_position.position,
                    ),
                    limit,
                )
                .map(|values| {
                    values
                        .into_iter()
                        .map(|(certificate, position)| {
                            (certificate, FetchCertificatesPosition::Target(position))
                        })
                        .collect()
                }),
        }
    }

    /// Fetch source head certificate for subnet
    ///
    /// Return position of the certificate and certificate itself
    pub async fn get_source_head(
        &self,
        subnet_id: SubnetId,
    ) -> Result<Option<(u64, Certificate)>, StorageError> {
        Ok(self.store.get_source_head(&subnet_id)?.and_then(|head| {
            self.store
                .get_certificate(&head.certificate_id)
                .ok()?
                .map(|certificate| (*head.position, certificate.certificate))
        }))
    }
}
