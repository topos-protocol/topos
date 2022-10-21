use std::time::{Instant, SystemTime};

use errors::InternalStorageError;
use serde::{Deserialize, Serialize};

use topos_core::uci::{Certificate, CertificateId, SubnetId};

pub(crate) mod command;
pub mod errors;

pub type PendingCertificateId = u64;

/// Certificate index in the history of the source subnet
pub type Height = u64;

/// Uniquely identify the tip of one subnet.
/// The tip represent the internal state of the TCE regarding a source subnet stream
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
        certificate: Certificate,
        status: CertificateStatus,
    ) -> Result<PendingCertificateId, InternalStorageError>;

    /// Update the certificate entry with new status
    async fn update(
        &self,
        certificate_id: &CertificateId,
        status: CertificateStatus,
    ) -> Result<(), InternalStorageError>;

    /// Returns the tips of given subnets
    async fn get_tip(&self, subnets: Vec<SubnetId>) -> Result<Vec<Tip>, InternalStorageError>;

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
    /// Ranged by height since emitted Certificate are totally ordered
    async fn get_certificates_by_source(
        &self,
        source_subnet_id: SubnetId,
        from: Height,
        to: Height,
    ) -> Result<Vec<CertificateId>, InternalStorageError>;

    /// Returns the certificate received by given subnet
    /// Ranged by timestamps since received Certificate are not referrable by height
    async fn get_certificates_by_target(
        &self,
        target_subnet_id: SubnetId,
        from: Instant,
        to: Instant,
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
