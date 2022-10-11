use std::time::{Instant, SystemTime};

use errors::InternalStorageError;
use serde::{Deserialize, Serialize};

use topos_core::uci::{Certificate, CertificateId, SubnetId};

pub mod client;
pub mod command;
pub mod connection;
pub mod errors;

#[cfg(feature = "inmemory")]
pub mod inmemory;
#[cfg(feature = "rocksdb")]
pub mod rocks;

#[cfg(test)]
mod tests;

pub use client::StorageClient;
pub use connection::Connection;

#[cfg(feature = "inmemory")]
pub use inmemory::InMemoryStorage;

#[cfg(feature = "rocksdb")]
pub use rocks::RocksDBStorage;

#[async_trait::async_trait]
pub trait Storage: Sync + Send + 'static {
    /// Add a pending certificate to the pool
    async fn add_pending_certificate(
        &mut self,
        certificate: Certificate,
    ) -> Result<(), InternalStorageError>;

    /// Persist the certificate with given status
    async fn persist(
        &mut self,
        certificate: Certificate,
        status: CertificateStatus,
    ) -> Result<(), InternalStorageError>;

    /// Update the certificate entry with new status
    async fn update(
        &mut self,
        certificate_id: &CertificateId,
        status: CertificateStatus,
    ) -> Result<(), InternalStorageError>;

    /// Returns the tips of given subnets
    async fn get_tip(&self, subnets: Vec<SubnetId>) -> Result<Vec<Tip>, InternalStorageError>;

    /// Returns the certificate data given their id
    async fn get_certificates(
        &self,
        cert_id: Vec<CertificateId>,
    ) -> Result<Vec<(CertificateStatus, Certificate)>, InternalStorageError>;

    /// Returns the certificate data given their id
    async fn get_certificate(
        &self,
        cert_id: CertificateId,
    ) -> Result<Certificate, InternalStorageError>;

    /// Returns the certificate emitted by given subnet
    /// Ranged by height since emitted Certificate are totally ordered
    async fn get_emitted_certificates(
        &self,
        subnet_id: SubnetId,
        from: Height,
        to: Height,
    ) -> Result<Vec<CertificateId>, InternalStorageError>;

    /// Returns the certificate received by given subnet
    /// Ranged by timestamps since received Certificate are not referrable by height
    async fn get_received_certificates(
        &self,
        subnet_id: SubnetId,
        from: Instant,
        to: Instant,
    ) -> Result<Vec<CertificateId>, InternalStorageError>;

    /// Returns all the known Certificate that are not delivered yet
    async fn get_pending_certificates(
        &self,
    ) -> Result<Vec<(u64, Certificate)>, InternalStorageError>;

    /// Remove a certificate from pending pool
    async fn remove_pending_certificate(&self, index: u64) -> Result<(), InternalStorageError>;
}

/// Certificate index in the history of its emitter subnet
pub type Height = u64;

/// Uniquely identify the tip of which subnet
#[derive(Serialize, Deserialize)]
pub struct Tip {
    /// Certificate id of the tip
    cert_id: CertificateId,
    /// Subnet id of the tip
    subnet_id: SubnetId,
    /// Height of the Certificate
    height: Height,
    /// Timestamp of the Certificate
    timestamp: SystemTime,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum CertificateStatus {
    Pending,
    Delivered,
}
