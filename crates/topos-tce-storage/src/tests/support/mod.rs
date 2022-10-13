use std::sync::atomic::AtomicU64;

use rstest::fixture;

use crate::{
    rocks::{
        CertificatesColumn, PendingCertificatesColumn, SourceSubnetStreamsColumn,
        TargetSubnetStreamsColumn,
    },
    RocksDBStorage,
};

use self::columns::{
    certificates_column, pending_column, source_streams_column, target_streams_column,
};

pub(crate) mod columns;
pub(crate) mod folder;

#[fixture]
pub(crate) fn storage(
    pending_column: PendingCertificatesColumn,
    certificates_column: CertificatesColumn,
    source_streams_column: SourceSubnetStreamsColumn,
    target_streams_column: TargetSubnetStreamsColumn,
) -> RocksDBStorage {
    RocksDBStorage::new(
        pending_column,
        certificates_column,
        source_streams_column,
        target_streams_column,
        AtomicU64::new(0),
    )
}
