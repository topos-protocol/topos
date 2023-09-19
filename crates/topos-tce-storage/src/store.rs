use std::collections::HashMap;

use async_trait::async_trait;
use topos_core::uci::{CertificateId, SubnetId};

use crate::{
    errors::StorageError,
    types::{CertificateDelivered, SourceStreamPositionKey},
    CertificatePositions, CertificateSourceStreamPosition, SourceHead,
};

#[async_trait]
pub trait WriteStore: Send {
    /// Insert a CertificateDelivered in the differents tables. Removing pending if needed.
    async fn insert_certificate_delivered(
        &self,
        certificate: &CertificateDelivered,
    ) -> Result<CertificatePositions, StorageError>;

    /// Insert multiple CertificateDelivered
    async fn multi_insert_certificates_delivered(
        &self,
        certificates: &[CertificateDelivered],
    ) -> Result<(), StorageError>;
}

pub trait ReadStore: Send {
    /// Try to get a Certificate
    fn get_certificate(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<CertificateDelivered>, StorageError>;

    /// Try to get multiple certificates at once
    fn multi_get_certificate(
        &self,
        certificate_ids: &[CertificateId],
    ) -> Result<Vec<Option<CertificateDelivered>>, StorageError>;

    /// Try to return the latest delivered position for a source subnet
    fn last_delivered_position_for_subnet(
        &self,
        subnet_id: &SubnetId,
    ) -> Result<Option<CertificateSourceStreamPosition>, StorageError>;

    /// Returns the local checkpoint
    fn get_checkpoint(&self) -> Result<HashMap<SubnetId, SourceHead>, StorageError>;

    /// Returns the certificates delivered by a source subnet from a position.
    fn get_source_stream_certificates_from_position(
        &self,
        from: SourceStreamPositionKey,
        limit: usize,
    ) -> Result<Vec<(CertificateDelivered, CertificateSourceStreamPosition)>, StorageError>;
}
