use std::{fs::create_dir_all, path::PathBuf, sync::atomic::AtomicU64};

use rocksdb::ColumnFamilyDescriptor;
use topos_core::{
    types::ProofOfDelivery,
    uci::{Certificate, CertificateId},
};
use tracing::warn;

use crate::{
    constant::cfs,
    rocks::{
        constants,
        db::{default_options, init_with_cfs},
        db_column::DBColumn,
    },
    types::{CertificatesColumn, EpochId, EpochSummary, PendingCertificatesColumn, StreamsColumn},
    PendingCertificateId,
};

/// Volatile and pending data used by Validator
///
/// It contains data that is not yet delivered.
///
/// When a [`Certificate`] is received, it can either be added to the pending
/// pool or to the precedence pool.
///
/// ## Pending pool
///
/// The pending pool is used to store certificates that are ready to be validated and broadcast.
/// Meaning that the previous [`Certificate`] has been delivered and the [`Certificate`] is
/// ready to be broadcast.
///
/// The ordering inside the pending pool is a FIFO queue, each [`Certificate`] in the pool gets
/// assigned to a unique [`PendingCertificateId`](type@crate::PendingCertificateId).
///
/// ## Precedence pool
///
/// The precedence pool is used to store certificates that are not yet ready to be broadcast,
/// mostly waiting for the previous certificate to be delivered. However, the [`Certificate`] is
/// already validated.
///
/// When a [`Certificate`] is delivered, the [`ValidatorStore`](struct@super::ValidatorStore) will
/// check for any [`Certificate`] in the precedence pool and if one is found, it is moved to the
/// pending pool, ready to be broadcast.
///
pub struct ValidatorPendingTables {
    pub(crate) next_pending_id: AtomicU64,
    pub(crate) pending_pool: PendingCertificatesColumn,
    pub(crate) pending_pool_index: DBColumn<CertificateId, PendingCertificateId>,
    pub(crate) precedence_pool: DBColumn<CertificateId, Certificate>,
}

impl ValidatorPendingTables {
    /// Open the [`ValidatorPendingTables`] at the given path.
    pub fn open(mut path: PathBuf) -> Self {
        path.push("pending");
        if !path.exists() {
            warn!("Path {:?} does not exist, creating it", path);
            create_dir_all(&path).expect("Cannot create ValidatorPendingTables directory");
        }
        let cfs = vec![
            ColumnFamilyDescriptor::new(cfs::PENDING_POOL, default_options()),
            ColumnFamilyDescriptor::new(cfs::PENDING_POOL_INDEX, default_options()),
            ColumnFamilyDescriptor::new(cfs::PRECEDENCE_POOL, default_options()),
        ];

        let db = init_with_cfs(&path, default_options(), cfs)
            .unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            // TODO: Fetch it from the storage
            next_pending_id: AtomicU64::new(0),
            pending_pool: DBColumn::reopen(&db, cfs::PENDING_POOL),
            pending_pool_index: DBColumn::reopen(&db, cfs::PENDING_POOL_INDEX),
            precedence_pool: DBColumn::reopen(&db, cfs::PRECEDENCE_POOL),
        }
    }
}

/// Data that shouldn't be purged at all.
// TODO: TP-774: Rename and move to FullNode domain
pub struct ValidatorPerpetualTables {
    pub(crate) certificates: CertificatesColumn,
    pub(crate) streams: StreamsColumn,
    #[allow(unused)]
    epoch_chain: DBColumn<EpochId, EpochSummary>,
    pub(crate) unverified: DBColumn<CertificateId, ProofOfDelivery>,
}

impl ValidatorPerpetualTables {
    pub fn open(mut path: PathBuf) -> Self {
        path.push("perpetual");
        if !path.exists() {
            warn!("Path {:?} does not exist, creating it", path);
            create_dir_all(&path).expect("Cannot create ValidatorPerpetualTables directory");
        }
        let mut options_stream = default_options();
        options_stream.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(
            constants::SOURCE_STREAMS_PREFIX_SIZE,
        ));

        let cfs = vec![
            ColumnFamilyDescriptor::new(cfs::CERTIFICATES, default_options()),
            ColumnFamilyDescriptor::new(cfs::STREAMS, options_stream),
            ColumnFamilyDescriptor::new(cfs::EPOCH_CHAIN, default_options()),
            ColumnFamilyDescriptor::new(cfs::UNVERIFIED, default_options()),
        ];

        let db = init_with_cfs(&path, default_options(), cfs).unwrap_or_else(|e| {
            panic!("Cannot open DB at {:?} => error {:?}", path, e);
        });

        Self {
            certificates: DBColumn::reopen(&db, cfs::CERTIFICATES),
            streams: DBColumn::reopen(&db, cfs::STREAMS),
            epoch_chain: DBColumn::reopen(&db, cfs::EPOCH_CHAIN),
            unverified: DBColumn::reopen(&db, cfs::UNVERIFIED),
        }
    }
}
