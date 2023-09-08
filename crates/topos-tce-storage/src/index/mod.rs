use std::path::PathBuf;

use rocksdb::ColumnFamilyDescriptor;
use topos_core::uci::{CertificateId, SubnetId};

use crate::{
    rocks::{
        constants,
        db::{default_options, init_with_cfs},
        db_column::DBColumn,
        TargetSourceListKey, TargetStreamPositionKey,
    },
    types::CertificateSequenceNumber,
    Position,
};

#[allow(unused)]
pub(crate) struct IndexStore {
    certificate_order: DBColumn<CertificateSequenceNumber, CertificateId>,
}

pub struct IndexTables {
    pub(crate) target_streams: DBColumn<TargetStreamPositionKey, CertificateId>,
    pub(crate) target_source_list: DBColumn<TargetSourceListKey, Position>,
    pub(crate) source_list: DBColumn<SubnetId, (CertificateId, Position)>,
    pub(crate) source_list_per_target: DBColumn<(SubnetId, SubnetId), bool>,
}

impl IndexTables {
    pub fn open(mut path: PathBuf) -> Self {
        path.push("index");
        let mut options_stream = default_options();
        options_stream.set_prefix_extractor(rocksdb::SliceTransform::create_fixed_prefix(
            constants::TARGET_STREAMS_PREFIX_SIZE,
        ));

        let cfs = vec![
            ColumnFamilyDescriptor::new("target_streams", options_stream),
            ColumnFamilyDescriptor::new("target_source_list", default_options()),
            ColumnFamilyDescriptor::new("source_list", default_options()),
            ColumnFamilyDescriptor::new(
                "delivered_certificates_per_source_for_target",
                default_options(),
            ),
        ];

        let db = init_with_cfs(&path, default_options(), cfs)
            .unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            target_streams: DBColumn::reopen(&db, "target_streams"),
            target_source_list: DBColumn::reopen(&db, "target_source_list"),
            source_list: DBColumn::reopen(&db, "source_list"),
            source_list_per_target: DBColumn::reopen(
                &db,
                "delivered_certificates_per_source_for_target",
            ),
        }
    }
}
