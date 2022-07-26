//!
//! Storage interface required to support TRB
//!
use tce_uci::{Certificate, CertificateId, DigestCompressed, SubnetId};

use crate::Errors;

/// Defines abstract storage suitable for protocol handler.
///
/// Implemented in node/store.
pub trait TrbStore {
    /// Saves (or replaces) the certificate
    fn apply_cert(&mut self, cert: &Certificate) -> Result<(), Errors>;

    /// Saves journal item in history
    fn add_cert_in_hist(&mut self, subnet_id: &SubnetId, cert_id: &Certificate) -> bool;

    /// Saves journal item in digest
    fn add_cert_in_digest(&mut self, subnet_id: &SubnetId, cert_id: &CertificateId) -> bool;

    /// Reads journal entries - from old to new, paged
    /// Returns tuple (data, last offset)
    fn read_journal(
        &self,
        subnet_id: SubnetId,
        from_offset: u64,
        max_results: u64,
    ) -> Result<(Vec<Certificate>, u64), Errors>;

    /// Easy access
    fn get_cert(&self, subnet_id: &SubnetId, last_n: u64) -> Option<Vec<CertificateId>>;

    /// Compute the digest for a given Subnet
    fn flush_digest_view(&mut self, subnet_id: &SubnetId) -> Option<DigestCompressed>;

    /// Read certificate
    fn cert_by_id(&self, cert_id: &CertificateId) -> Result<Option<Certificate>, Errors>;

    /// Receive new cert from broadcast along with digest
    fn new_cert_candidate(&mut self, cert: &Certificate, digest: &DigestCompressed);

    /// Check on the digest
    fn check_digest_inclusion(&self, cert: &Certificate) -> Result<(), Errors>;

    /// Check on the previous cert
    fn check_precedence(&self, cert: &Certificate) -> Result<(), Errors>;

    fn clone_box(&self) -> Box<dyn TrbStore + Send>;
}
