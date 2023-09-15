use std::path::PathBuf;

use rocksdb::ColumnFamilyDescriptor;
use topos_core::{
    types::stream::{CertificateTargetStreamPosition, Position},
    uci::{CertificateId, SubnetId},
};

use crate::{
    constant::cfs,
    rocks::{
        constants,
        db::{default_options, init_with_cfs},
        db_column::DBColumn,
        TargetSourceListKey,
    },
    types::CertificateSequenceNumber,
};

#[allow(unused)]
pub(crate) struct IndexStore {
    certificate_order: DBColumn<CertificateSequenceNumber, CertificateId>,
}

pub struct IndexTables {
    pub(crate) target_streams: DBColumn<CertificateTargetStreamPosition, CertificateId>,
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
            ColumnFamilyDescriptor::new(cfs::TARGET_STREAMS, options_stream),
            ColumnFamilyDescriptor::new(cfs::TARGET_SOURCE_LIST, default_options()),
            ColumnFamilyDescriptor::new(cfs::SOURCE_LIST, default_options()),
            ColumnFamilyDescriptor::new(
                cfs::DELIVERED_CERTIFICATES_PER_SOURCE_FOR_TARGET,
                default_options(),
            ),
        ];

        let db = init_with_cfs(&path, default_options(), cfs)
            .unwrap_or_else(|_| panic!("Cannot open DB at {:?}", path));

        Self {
            target_streams: DBColumn::reopen(&db, cfs::TARGET_STREAMS),
            target_source_list: DBColumn::reopen(&db, cfs::TARGET_SOURCE_LIST),
            source_list: DBColumn::reopen(&db, cfs::SOURCE_LIST),
            source_list_per_target: DBColumn::reopen(
                &db,
                cfs::DELIVERED_CERTIFICATES_PER_SOURCE_FOR_TARGET,
            ),
        }
    }
}
