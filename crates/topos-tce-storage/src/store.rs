use std::collections::HashMap;

use async_trait::async_trait;
use topos_core::{
    types::{stream::CertificateSourceStreamPosition, CertificateDelivered},
    uci::{CertificateId, SubnetId},
};

use crate::{
    errors::StorageError, CertificatePositions, CertificateTargetStreamPosition, SourceHead,
};

/// This trait exposes common methods between
/// [`ValidatorStore`](struct@super::validator::ValidatorStore) and
/// [`FullNodeStore`](struct@super::fullnode::FullNodeStore) to write data.
///
/// All methods are `async` to allow the implementation to deal with write concurrency.
#[async_trait]
pub trait WriteStore: Send {
    /// Insert a [`CertificateDelivered`] in the storage. Returns its positions
    /// in the source and target streams.
    ///
    /// The [`ValidatorStore`](struct@super::validator::ValidatorStore) implementation
    /// checks for a [`PendingCertificateId`](type@super::PendingCertificateId) and remove it if
    /// the certificate is successfully inserted.
    async fn insert_certificate_delivered(
        &self,
        certificate: &CertificateDelivered,
    ) -> Result<CertificatePositions, StorageError>;

    /// Insert multiple [`CertificateDelivered`] in the storage.
    ///
    /// See [`insert_certificate_delivered`](fn@WriteStore::insert_certificate_delivered) for more
    /// details
    async fn insert_certificates_delivered(
        &self,
        certificates: &[CertificateDelivered],
    ) -> Result<(), StorageError>;
}

/// This trait exposes common methods between
/// [`ValidatorStore`](struct@super::validator::ValidatorStore) and
/// [`FullNodeStore`](struct@super::fullnode::FullNodeStore) to read data.
pub trait ReadStore: Send {
    /// Returns the number of certificates delivered
    fn count_certificates_delivered(&self) -> Result<usize, StorageError>;

    /// Try to get a SourceHead of a subnet
    ///
    /// Returns `Ok(None)` if the subnet is not found, meaning that no certificate are currently
    /// delivered for this particular subnet.
    fn get_source_head(&self, subnet_id: &SubnetId) -> Result<Option<SourceHead>, StorageError>;

    /// Try to get a [`CertificateDelivered`]
    ///
    /// Returns `Ok(None)` if the certificate is not found, meaning that the certificate is either
    /// inexisting or not yet delivered.
    fn get_certificate(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<Option<CertificateDelivered>, StorageError>;

    /// Try to get multiple [`CertificateDelivered`] at once.
    ///
    /// See [`get_certificate`](fn@ReadStore::get_certificate)
    fn get_certificates(
        &self,
        certificate_ids: &[CertificateId],
    ) -> Result<Vec<Option<CertificateDelivered>>, StorageError>;

    /// Try to return the latest delivered position for a source subnet
    fn last_delivered_position_for_subnet(
        &self,
        subnet_id: &SubnetId,
    ) -> Result<Option<CertificateSourceStreamPosition>, StorageError>;

    /// Returns the local checkpoint
    ///
    /// A `Checkpoint` is the representation of the state of delivery, it is a list of [`SubnetId`]
    /// with the associated [`SourceHead`]
    fn get_checkpoint(&self) -> Result<HashMap<SubnetId, SourceHead>, StorageError>;

    /// Returns the certificates delivered by a source subnet from a position.
    fn get_source_stream_certificates_from_position(
        &self,
        from: CertificateSourceStreamPosition,
        limit: usize,
    ) -> Result<Vec<(CertificateDelivered, CertificateSourceStreamPosition)>, StorageError>;

    /// Returns the certificates delivered to a target subnet from a position.
    fn get_target_stream_certificates_from_position(
        &self,
        position: CertificateTargetStreamPosition,
        limit: usize,
    ) -> Result<Vec<(CertificateDelivered, CertificateTargetStreamPosition)>, StorageError>;

    /// Returns the list of source subnets that delivered certificates to a particular target subnet
    fn get_target_source_subnet_list(
        &self,
        target_subnet_id: &SubnetId,
    ) -> Result<Vec<SubnetId>, StorageError>;
}
