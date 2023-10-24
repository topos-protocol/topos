//! The library provides the storage layer for the Topos TCE.
//! It is responsible for storing and retrieving the certificates, managing the
//! pending certificates pool and the certificate status, storing the different
//! metadata related to the protocol and the internal state of the TCE.
//!
//! The storage layer is implemented using RocksDB.
//! The library is exposing multiple store that are used by the TCE.
//!
//!
//! ## Architecture
//!
//! The storage layer is composed of multiple stores that are used by the TCE.
//! Each store is describe in detail in its own module.
//!
//! As an overview, the storage layer is composed of the following stores:
//!
//!<picture>
//!  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/topos-protocol/topos/assets/1394604/5bb3c9b1-ac5a-4f59-bd14-29a02163272e">
//!  <img alt="Text changing depending on mode. Light: 'So light!' Dark: 'So dark!'" src="https://github.com/topos-protocol/topos/assets/1394604/e4bd859e-2a6d-40dc-8e84-2a708aa8a2d8">
//!</picture>
//!
//! ## Usage
//!
//! Each store represents a different kind of capabilities, but they all act and need the same kind
//! of configuration in order to work.
//!
//! For instance, the [`EpochValidatorsStore`](struct@epoch::EpochValidatorsStore) only needs a [`PathBuf`](struct@std::path::PathBuf)
//! argument to be instantiated where [`FullNodeStore`](struct@fullnode::FullNodeStore) needs a little bit more arguments.
//!
//! The underlying mechanisms of how data is stored is fairly simple, it relies a lot on [`rocksdb`] and will
//! be describe below.
//!
//! As an example, in order to create a new [`EpochValidatorsStore`](struct@epoch::EpochValidatorsStore) you need to provide a
//! path where the [`rocksdb`] database will be placed:
//!
//! ```
//! # use topos_tce_storage::epoch;
//! use epoch::EpochValidatorsStore;
//! # use std::str::FromStr;
//! # use std::path::PathBuf;
//! # use std::sync::Arc;
//! # let mut path = PathBuf::from_str(env!("CARGO_MANIFEST_DIR")).unwrap();
//! # path.push("./../../target/tmp/");
//! path.push("epoch");
//! let store: Arc<EpochValidatorsStore> = EpochValidatorsStore::new(path).unwrap();
//! ```
//!
//! ## Special Considerations
//!
//! When using the storage layer, you need to be aware of the following:
//! - The storage layer is using [`rocksdb`] as a backend, which means that the data is stored on disk.
//! - The storage layer is using [`Arc`](struct@std::sync::Arc) to share the stores between threads.
//! - The storage layer is using [`async_trait`](https://docs.rs/async-trait/0.1.51/async_trait/) to expose methods that need to manage locks. (see [`WriteStore`](trait@store::WriteStore))
//! - Some functions are using [`DBBatch`](struct@rocks::db_column::DBBatch) to batch multiple writes in one transaction. But not all functions are using it.
//!
//! ## Design Philosophy
//!
//! The choice of using [`rocksdb`] as a backend was made because it is a well known and battle tested database.
//! It is also very fast and efficient when it comes to write and read data. However, it is not the best when it comes
//! to compose or filter data. This is why we have multiple store that are used for different purposes.
//!
//! For complex queries, another database like [`PostgreSQL`](https://www.postgresql.org/) or [`CockroachDB`](https://www.cockroachlabs.com/) could be used as a Storage for projections.
//! The source of truth would still be [`rocksdb`] but the projections would be stored in a relational database. Allowing for more complex queries.
//!
//! As mention above, the different stores are using [`Arc`](struct@std::sync::Arc), allowing a single store to be instantiated once
//! and then shared between threads. This is very useful when it comes to the [`FullNodeStore`](struct@fullnode::FullNodeStore) as it is used in various places.
//!
//! It also means that the store is immutable, which is a good thing when it comes to concurrency.
//! The burden of managing the locks is handled by the [`async_trait`](https://docs.rs/async-trait/0.1.51/async_trait/) crate when using the [`WriteStore`](trait@store::WriteStore).
//! The rest of the mutation on the data are handled by [`rocksdb`] itself.
//!
use errors::InternalStorageError;
use rocks::iterator::ColumnIterator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use topos_core::{
    types::stream::{CertificateSourceStreamPosition, CertificateTargetStreamPosition, Position},
    uci::{Certificate, CertificateId, SubnetId},
};

// v2
pub mod constant;
/// Epoch related store
pub mod epoch;
/// Fullnode store
pub mod fullnode;
pub mod index;
pub mod types;
/// Everything that is needed to participate to the protocol
pub mod validator;

// v1
pub mod client;
pub mod errors;

#[cfg(feature = "rocksdb")]
pub(crate) mod rocks;

#[cfg(test)]
mod tests;

pub use client::StorageClient;

pub mod store;

pub type PendingCertificateId = u64;

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
    ) -> Result<
        ColumnIterator<'_, CertificateTargetStreamPosition, CertificateId>,
        InternalStorageError,
    >;

    async fn get_source_list_by_target(
        &self,
        target: SubnetId,
    ) -> Result<Vec<SubnetId>, InternalStorageError>;
}
