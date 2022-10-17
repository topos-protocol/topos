use std::path::PathBuf;

use rstest::fixture;

use crate::rocks::{
    constants, db_column::DBColumn, CertificatesColumn, PendingCertificatesColumn,
    SourceSubnetStreamsColumn, TargetSubnetStreamsColumn,
};

use super::folder::created_folder;

#[fixture]
pub(crate) fn pending_column(created_folder: Box<PathBuf>) -> PendingCertificatesColumn {
    DBColumn::open(*created_folder, None, constants::PENDING_CERTIFICATES).unwrap()
}

#[fixture]
pub(crate) fn certificates_column(created_folder: Box<PathBuf>) -> CertificatesColumn {
    DBColumn::open(*created_folder, None, constants::CERTIFICATES).unwrap()
}

#[fixture]
pub(crate) fn source_streams_column(created_folder: Box<PathBuf>) -> SourceSubnetStreamsColumn {
    DBColumn::open(*created_folder, None, constants::SOURCE_SUBNET_STREAMS).unwrap()
}

#[fixture]
pub(crate) fn target_streams_column(created_folder: Box<PathBuf>) -> TargetSubnetStreamsColumn {
    DBColumn::open(*created_folder, None, constants::TARGET_SUBNET_STREAMS).unwrap()
}
