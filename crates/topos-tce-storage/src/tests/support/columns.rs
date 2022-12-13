use rstest::fixture;

use crate::rocks::{
    constants, db_column::DBColumn, CertificatesColumn, PendingCertificatesColumn,
    SourceStreamsColumn, TargetStreamsColumn,
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
pub(crate) fn source_streams_column(database_name: &'static str) -> SourceStreamsColumn {
    DBColumn::reopen(&rocks_db(database_name), constants::SOURCE_STREAMS)
}

#[fixture]
pub(crate) fn target_streams_column(database_name: &'static str) -> TargetStreamsColumn {
    DBColumn::reopen(&rocks_db(database_name), constants::TARGET_STREAMS)
}
