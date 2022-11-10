use rstest::fixture;

use crate::rocks::{
    constants, db_column::DBColumn, CertificatesColumn, PendingCertificatesColumn,
    SourceSubnetStreamsColumn, TargetSubnetStreamsColumn,
};

use super::database_name;
use super::rocks_db;

#[fixture]
pub(crate) fn pending_column(database_name: &'static str) -> PendingCertificatesColumn {
    DBColumn::reopen(&rocks_db(database_name), constants::PENDING_CERTIFICATES)
}

#[fixture]
pub(crate) fn certificates_column(database_name: &'static str) -> CertificatesColumn {
    DBColumn::reopen(&rocks_db(database_name), constants::CERTIFICATES)
}

#[fixture]
pub(crate) fn source_streams_column(database_name: &'static str) -> SourceSubnetStreamsColumn {
    DBColumn::reopen(&rocks_db(database_name), constants::SOURCE_SUBNET_STREAMS)
}

#[fixture]
pub(crate) fn target_streams_column(database_name: &'static str) -> TargetSubnetStreamsColumn {
    DBColumn::reopen(&rocks_db(database_name), constants::TARGET_SUBNET_STREAMS)
}
