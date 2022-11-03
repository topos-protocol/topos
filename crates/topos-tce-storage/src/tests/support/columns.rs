use rstest::fixture;

use crate::rocks::{
    constants, db::RocksDB, db_column::DBColumn, CertificatesColumn, PendingCertificatesColumn,
    SourceSubnetStreamsColumn, TargetSubnetStreamsColumn,
};

use super::rocks_db;

#[fixture]
pub(crate) fn pending_column(rocks_db: &RocksDB) -> PendingCertificatesColumn {
    DBColumn::reopen(rocks_db, constants::PENDING_CERTIFICATES)
}

#[fixture]
pub(crate) fn certificates_column(rocks_db: &RocksDB) -> CertificatesColumn {
    DBColumn::reopen(rocks_db, constants::CERTIFICATES)
}

#[fixture]
pub(crate) fn source_streams_column(rocks_db: &RocksDB) -> SourceSubnetStreamsColumn {
    DBColumn::reopen(rocks_db, constants::SOURCE_SUBNET_STREAMS)
}

#[fixture]
pub(crate) fn target_streams_column(rocks_db: &RocksDB) -> TargetSubnetStreamsColumn {
    DBColumn::reopen(rocks_db, constants::TARGET_SUBNET_STREAMS)
}
