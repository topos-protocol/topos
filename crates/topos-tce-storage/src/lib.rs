//! The library provides the storage layer for the Topos TCE.
//! It is responsible for storing and retrieving the [certificates](https://docs.topos.technology/content/module-1/4-protocol.html#certificates), managing the
//! pending certificates pool and the certificate status, storing different
//! metadata related to the protocol and the internal state of the TCE.
//!
//! The storage layer is implemented using RocksDB.
//! The library exposes multiple stores that are used by the TCE.
//!
//!
//! ## Architecture
//!
//! The storage layer is composed of multiple stores that are used by the TCE.
//! Each store is described in detail in its own module.
//!
//! Those stores are mainly used in `topos-tce-broadcast`, `topos-tce-api` and
//! `topos-tce-synchronizer`.
//!
//! As an overview, the storage layer is composed of the following stores:
//!
//!<picture>
//!  <source media="(prefers-color-scheme: dark)" srcset="https://github.com/topos-protocol/topos/assets/1394604/5bb3c9b1-ac5a-4f59-bd14-29a02163272e">
//!  <img alt="Text changing depending on mode. Light: 'So light!' Dark: 'So dark!'" src="https://github.com/topos-protocol/topos/assets/1394604/e4bd859e-2a6d-40dc-8e84-2a708aa8a2d8">
//!</picture>
//!
//! ### Definitions and Responsibilities
//!
//! As illustrated above, multiple `stores` are exposed in the library using various `tables`.
//!
//! The difference between a `store` and a `table` is that the `table` is responsible for storing
//! the data while the `store` manages the data access and its behavior.
//!
//! Here's the list of the different stores and their responsibilities:
//!
//! - The [`EpochValidatorsStore`](struct@epoch::EpochValidatorsStore) is responsible for managing the list of validators for each `epoch`.
//! - The [`FullNodeStore`](struct@fullnode::FullNodeStore) is responsible for managing all persistent data such as [`Certificate`](struct@topos_core::uci::Certificate) delivered and associated `streams`.
//! - The [`IndexStore`](struct@index::IndexStore) is responsible for managing indexes and collect information about the broadcast and the network.
//! - The [`ValidatorStore`](struct@validator::ValidatorStore) is responsible for managing the pending data that one validator needs to keep track, such as the certificates pool.
//!
//! For more information about a `store`, see the related doc.
//!
//! Next, we've the list of the different tables and their responsibilities:
//!
//! - The [`EpochValidatorsTables`](struct@epoch::EpochValidatorsTables) is responsible for storing the list of validators for each `epoch`.
//! - The [`ValidatorPerpetualTables`](struct@validator::ValidatorPerpetualTables) is responsible for storing the delivered [`Certificate`](struct@topos_core::uci::Certificate)s and the persistent data related to the Broadcast.
//! - The [`ValidatorPendingTables`](struct@validator::ValidatorPendingTables) is responsible for storing the pending data, such as the certificates pool.
//! - The [`IndexTables`](struct@index::IndexTables) is responsible for storing indexes about the delivery of [`Certificate`](struct@topos_core::uci::Certificate)s such as `target subnet stream`.
//!
//! ## Special Considerations
//!
//! When using the storage layer, be aware of the following:
//! - The storage layer uses [rocksdb](https://rocksdb.org/) as the backend, which means don't need an external service, as `rocksdb` is an embedded key-value store.
//! - The storage layer uses [`Arc`](struct@std::sync::Arc) to share the stores between threads. It also means that a `store` is only instantiated once.
//! - Some storage methods are batching multiple writes into a single transaction.
//!
//! ## Design Philosophy
//!
//! The choice of using [rocksdb](https://rocksdb.org/) as a backend was made because it matches a lot of the conditions
//! that we were expected, such as being embedded and having good performances when reading and
//! writing our data.
//!
//! Splitting storage into multiple `stores` and `tables` allows us to have a strong separation of concerns directly at the storage level.
//!
//! However, `RocksDB` is not the best fit when it comes to compose or filter data based on the data
//! itself.
//!
//! As mentioned above, the different stores are using [`Arc`](struct@std::sync::Arc), allowing a single store to be instantiated once
//! and then shared between threads. This is very useful when it comes to the [`FullNodeStore`](struct@fullnode::FullNodeStore) as it is used
//! in various places but should provide single entry point to the data.
//!
//! It also means that the store is immutable thus can be shared easily between threads,
//! which is a good thing for the concurrency.
//! However, some stores are implementing the [`WriteStore`](trait@store::WriteStore) trait in order to
//! insert or mutate data, managing locks on resources and preventing any other query to mutate the data
//! currently in processing. For more information about the locks see [`locking`](module@fullnode::locking)
//!
//! The rest of the mutation on the data are handled by [rocksdb](https://rocksdb.org/) itself.
//!
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use topos_core::{
    types::stream::{CertificateSourceStreamPosition, CertificateTargetStreamPosition, Position},
    uci::{CertificateId, SubnetId},
};

// v2
pub mod constant;
/// Epoch related store
pub mod epoch;
/// Fullnode store
pub mod fullnode;
pub mod index;
pub mod types;
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
/// certificates that it receives from the local certificate producer
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SourceHead {
    /// Certificate id of the head
    pub certificate_id: CertificateId,
    /// Subnet id of the head
    pub subnet_id: SubnetId,
    /// Position of the Certificate
    pub position: Position,
}
