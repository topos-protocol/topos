use errors::{InternalStorageError, PositionError};
use serde::{Deserialize, Serialize};

use topos_core::uci::{Certificate, CertificateId};

pub mod client;
pub(crate) mod command;
pub(crate) mod connection;
pub mod errors;
pub mod events;

#[cfg(feature = "rocksdb")]
pub(crate) mod rocks;

#[cfg(test)]
mod tests;

pub use client::StorageClient;
pub use connection::Connection;

#[cfg(feature = "rocksdb")]
pub use rocks::RocksDBStorage;

pub type PendingCertificateId = u64;

#[derive(Debug)]
pub enum FetchCertificatesFilter {
    Source {
        subnet_id: SubnetId,
        version: u64,
        limit: usize,
    },

    Target {
        target_subnet_id: SubnetId,
        source_subnet_id: SubnetId,
        version: u64,
        limit: usize,
    },
}

#[derive(Debug, Serialize, Clone, Copy, Deserialize)]
// TODO: Replace in UCI
pub struct SubnetId {
    inner: [u8; 32],
}

impl TryFrom<&str> for SubnetId {
    type Error = InternalStorageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() != 32 {
            return Err(InternalStorageError::InvalidSubnetId);
        }

        Ok(Self {
            inner: value
                .as_bytes()
                .try_into()
                .map_err(|_| InternalStorageError::InvalidSubnetId)?,
        })
    }
}

impl From<[u8; 32]> for SubnetId {
    fn from(inner: [u8; 32]) -> Self {
        Self { inner }
    }
}

impl TryFrom<SubnetId> for [u8; 32] {
    type Error = ();
    fn try_from(subnet_id: SubnetId) -> Result<Self, Self::Error> {
        Ok(subnet_id.inner)
    }
}

impl ToString for SubnetId {
    fn to_string(&self) -> String {
        String::from_utf8_lossy(&self.inner).to_string()
    }
}

/// Certificate index in the history of the source subnet
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position(pub(crate) u64);

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

/// Uniquely identify the source certificate stream head of one subnet.
/// The head represent the internal state of the TCE regarding a source subnet stream for
/// certificates that it receives from local sequencer
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SourceHead {
    /// Certificate id of the head
    cert_id: CertificateId,
    /// Subnet id of the head
    subnet_id: SubnetId,
    /// Position of the Certificate
    position: Position,
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
    /// Add a pending certificate to the pool
    async fn add_pending_certificate(
        &self,
        certificate: Certificate,
    ) -> Result<PendingCertificateId, InternalStorageError>;

    /// Persist the certificate with given status
    async fn persist(
        &self,
        certificate: &Certificate,
        pending_certificate_id: Option<PendingCertificateId>,
    ) -> Result<(), InternalStorageError>;

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

    /// Remove a certificate from pending pool
    async fn remove_pending_certificate(
        &self,
        index: PendingCertificateId,
    ) -> Result<(), InternalStorageError>;
}
