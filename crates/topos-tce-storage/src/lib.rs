use errors::{InternalStorageError, PositionError};
use rocks::SourceStreamPositionKey;
use rocks::{iterator::ColumnIterator, TargetStreamPositionKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use topos_core::uci::{Certificate, CertificateId, SubnetId};

// v2
/// Everything that is needed to participate to the protocol
pub mod authority;
/// Epoch related store
pub mod epoch;
/// Fullnode store
pub mod fullnode;
pub mod index;
pub mod types;

// v1
pub mod client;
pub(crate) mod command;
pub(crate) mod connection;
mod constant;
pub mod errors;
pub mod events;

#[cfg(feature = "rocksdb")]
pub(crate) mod rocks;

#[cfg(test)]
mod tests;

pub use client::StorageClient;
pub use connection::Connection;
pub use connection::ConnectionBuilder;

#[cfg(feature = "rocksdb")]
pub use rocks::RocksDBStorage;

pub mod store;

pub type PendingCertificateId = u64;

/// Certificate index in the history of the source subnet
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub struct Position(pub u64);

impl Position {
    const ZERO: Self = Self(0);

    pub(crate) fn increment(self) -> Result<Self, PositionError> {
        match self {
            Self::ZERO => Ok(Self(1)),
            Self(value) => value
                .checked_add(1)
                .ok_or(PositionError::MaximumPositionReached)
                .map(Self),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateSourceStreamPosition {
    pub source_subnet_id: SubnetId,
    pub position: Position,
}

impl From<SourceStreamPositionKey> for CertificateSourceStreamPosition {
    fn from(value: SourceStreamPositionKey) -> Self {
        CertificateSourceStreamPosition {
            source_subnet_id: value.0,
            position: value.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CertificateTargetStreamPosition {
    pub target_subnet_id: SubnetId,
    pub source_subnet_id: SubnetId,
    pub position: Position,
}

#[derive(Debug)]
pub enum FetchCertificatesFilter {
    Source {
        source_stream_position: CertificateSourceStreamPosition,
        limit: usize,
    },

    Target {
        target_stream_position: CertificateTargetStreamPosition,
        limit: usize,
    },
}

#[derive(Debug)]
pub enum FetchCertificatesPosition {
    Source(CertificateSourceStreamPosition),
    Target(CertificateTargetStreamPosition),
}

#[derive(Debug, Clone)]
pub struct CertificatePositions {
    pub targets: HashMap<SubnetId, CertificateTargetStreamPosition>,
    pub source: CertificateSourceStreamPosition,
}

/// Uniquely identify the source certificate stream head of one subnet.
/// The head represent the internal state of the TCE regarding a source subnet stream for
/// certificates that it receives from local sequencer
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SourceHead {
    /// Certificate id of the head
    pub certificate_id: CertificateId,
    /// Subnet id of the head
    pub subnet_id: SubnetId,
    /// Position of the Certificate
    pub position: Position,
}

/// Define possible status of a certificate
#[derive(Debug, Deserialize, Serialize)]
pub enum CertificateStatus {
    Pending,
    Delivered,
}

/// The `Storage` trait defines methods to interact and manage with the persistency layer
#[async_trait::async_trait]
pub trait Storage: Sync + Send + 'static {
    async fn get_pending_certificate(
        &self,
        certificate_id: CertificateId,
    ) -> Result<(PendingCertificateId, Certificate), InternalStorageError>;

    /// Add a pending certificate to the pool
    async fn add_pending_certificate(
        &self,
        certificate: &Certificate,
    ) -> Result<PendingCertificateId, InternalStorageError>;

    /// Persist the certificate with given status
    async fn persist(
        &self,
        certificate: &Certificate,
        pending_certificate_id: Option<PendingCertificateId>,
    ) -> Result<CertificatePositions, InternalStorageError>;

    /// Update the certificate entry with new status
    async fn update(
        &self,
        certificate_id: &CertificateId,
        status: CertificateStatus,
    ) -> Result<(), InternalStorageError>;

    /// Returns the source heads of given subnets
    async fn get_source_heads(
        &self,
        subnets: Vec<SubnetId>,
    ) -> Result<Vec<crate::SourceHead>, InternalStorageError>;

    /// Returns the certificate data given their id
    async fn get_certificates(
        &self,
        certificate_ids: Vec<CertificateId>,
    ) -> Result<Vec<Certificate>, InternalStorageError>;

    /// Returns the certificate data given its id
    async fn get_certificate(
        &self,
        certificate_id: CertificateId,
    ) -> Result<Certificate, InternalStorageError>;

    /// Returns the certificate emitted by given subnet
    /// Ranged by position since emitted Certificate are totally ordered
    async fn get_certificates_by_source(
        &self,
        source_subnet_id: SubnetId,
        from: Position,
        limit: usize,
    ) -> Result<Vec<CertificateId>, InternalStorageError>;

    /// Returns the certificate received by given subnet
    /// Ranged by timestamps since received Certificate are not referrable by position
    async fn get_certificates_by_target(
        &self,
        target_subnet_id: SubnetId,
        source_subnet_id: SubnetId,
        from: Position,
        limit: usize,
    ) -> Result<Vec<CertificateId>, InternalStorageError>;

    /// Returns all the known Certificate that are not delivered yet
    async fn get_pending_certificates(
        &self,
    ) -> Result<Vec<(PendingCertificateId, Certificate)>, InternalStorageError>;

    /// Returns the next Certificate that are not delivered yet
    async fn get_next_pending_certificate(
        &self,
        starting_at: Option<usize>,
    ) -> Result<Option<(PendingCertificateId, Certificate)>, InternalStorageError>;

    /// Remove a certificate from pending pool
    async fn remove_pending_certificate(
        &self,
        index: PendingCertificateId,
    ) -> Result<(), InternalStorageError>;

    async fn get_target_stream_iterator(
        &self,
        target: SubnetId,
        source: SubnetId,
        position: Position,
    ) -> Result<ColumnIterator<'_, TargetStreamPositionKey, CertificateId>, InternalStorageError>;

    async fn get_source_list_by_target(
        &self,
        target: SubnetId,
    ) -> Result<Vec<SubnetId>, InternalStorageError>;
}
