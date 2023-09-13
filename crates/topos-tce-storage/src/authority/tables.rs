use std::path::PathBuf;

use rocksdb::ColumnFamilyDescriptor;
use topos_core::uci::{Certificate, CertificateId};

use crate::{
    rocks::{
        constants,
        db::{default_options, init_with_cfs},
        db_column::DBColumn,
        SourceStreamPositionKey,
    },
    types::{CertificateDelivered, EpochId, EpochSummary, ProofOfDelivery},
    PendingCertificateId,
};

/// Volatile and pending data
pub struct AuthorityPendingTables {
    pub(crate) pending_pool: DBColumn<PendingCertificateId, Certificate>,
    pub(crate) pending_pool_index: DBColumn<CertificateId, PendingCertificateId>,
    #[allow(unused)]
    precedence_pool: DBColumn<CertificateId, Vec<Certificate>>,
    #[allow(unused)]
    expiration_tracker: (), // Unknown
}
impl AuthorityPendingTables {
    pub fn open(mut path: PathBuf) -> Self {
        path.push("pending");

        let cfs = vec![
            ColumnFamilyDescriptor::new("pending_pool", default_options()),
            ColumnFamilyDescriptor::new("precedence_pool", default_options()),
        ];

        let db = init_with_cfs(&path, default_options(), cfs)
            .unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            pending_pool: DBColumn::reopen(&db, "pending_pool"),
            pending_pool_index: DBColumn::reopen(&db, "pending_pool_index"),
            precedence_pool: DBColumn::reopen(&db, "precedence_pool"),
            expiration_tracker: (),
        }
    }
}

/// Data that shouldn't be purged at all.
pub struct AuthorityPerpetualTables {
    pub(crate) certificates: DBColumn<CertificateId, CertificateDelivered>,
    pub(crate) streams: DBColumn<SourceStreamPositionKey, CertificateId>,
    #[allow(unused)]
    epoch_chain: DBColumn<EpochId, EpochSummary>,
    pub(crate) unverified: DBColumn<CertificateId, ProofOfDelivery>,
}

impl AuthorityPerpetualTables {
    pub fn open(mut path: PathBuf) -> Self {
        path.push("perpetual");
        let mut options_stream = default_options();
        options_stream.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(
            constants::SOURCE_STREAMS_PREFIX_SIZE,
        ));

        let cfs = vec![
            ColumnFamilyDescriptor::new("certificates", default_options()),
            ColumnFamilyDescriptor::new("streams", options_stream),
            ColumnFamilyDescriptor::new("epoch_chain", default_options()),
            ColumnFamilyDescriptor::new("unverified", default_options()),
        ];

        let db = init_with_cfs(&path, default_options(), cfs)
            .unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            certificates: DBColumn::reopen(&db, "certificates"),
            streams: DBColumn::reopen(&db, "streams"),
            epoch_chain: DBColumn::reopen(&db, "epoch_chain"),
            unverified: DBColumn::reopen(&db, "unverified"),
        }
    }
}
