use std::{collections::BTreeSet, path::PathBuf, sync::atomic::AtomicU64};

use rocksdb::ColumnFamilyDescriptor;
use topos_core::{
    types::{stream::CertificateSourceStreamPosition, CertificateDelivered, ProofOfDelivery},
    uci::{Certificate, CertificateId},
};

use crate::{
    constant::cfs,
    rocks::{
        constants,
        db::{default_options, init_with_cfs},
        db_column::DBColumn,
    },
    types::{EpochId, EpochSummary},
    PendingCertificateId,
};

/// Volatile and pending data
pub struct ValidatorPendingTables {
    pub(crate) next_pending_id: AtomicU64,
    #[allow(unused)]
    fetching_pool: BTreeSet<CertificateId>, // Not sure to keep it
    pub(crate) pending_pool: DBColumn<PendingCertificateId, Certificate>,
    pub(crate) pending_pool_index: DBColumn<CertificateId, PendingCertificateId>,
    pub(crate) precedence_pool: DBColumn<CertificateId, Certificate>,
    #[allow(unused)]
    expiration_tracker: (), // Unknown
}

impl ValidatorPendingTables {
    pub fn open(mut path: PathBuf) -> Self {
        path.push("pending");

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
            fetching_pool: BTreeSet::new(),
            pending_pool: DBColumn::reopen(&db, cfs::PENDING_POOL),
            pending_pool_index: DBColumn::reopen(&db, cfs::PENDING_POOL_INDEX),
            precedence_pool: DBColumn::reopen(&db, cfs::PRECEDENCE_POOL),
            expiration_tracker: (),
        }
    }
}

/// Data that shouldn't be purged at all.
pub struct ValidatorPerpetualTables {
    pub(crate) certificates: DBColumn<CertificateId, CertificateDelivered>,
    pub(crate) streams: DBColumn<CertificateSourceStreamPosition, CertificateId>,
    #[allow(unused)]
    epoch_chain: DBColumn<EpochId, EpochSummary>,
    pub(crate) unverified: DBColumn<CertificateId, ProofOfDelivery>,
}

impl ValidatorPerpetualTables {
    pub fn open(mut path: PathBuf) -> Self {
        path.push("perpetual");
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

        let db = init_with_cfs(&path, default_options(), cfs)
            .unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            certificates: DBColumn::reopen(&db, cfs::CERTIFICATES),
            streams: DBColumn::reopen(&db, cfs::STREAMS),
            epoch_chain: DBColumn::reopen(&db, cfs::EPOCH_CHAIN),
            unverified: DBColumn::reopen(&db, cfs::UNVERIFIED),
        }
    }
}
