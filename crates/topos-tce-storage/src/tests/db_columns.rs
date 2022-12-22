use rstest::rstest;
use topos_core::uci::Certificate;

use crate::tests::{PREV_CERTIFICATE_ID, SOURCE_STORAGE_SUBNET_ID, SOURCE_SUBNET_ID};
use crate::{
    rocks::{
        map::Map, CertificatesColumn, PendingCertificatesColumn, SourceStreamPosition,
        SourceStreamsColumn,
    },
    Position,
};

use super::support::columns::{certificates_column, pending_column, source_streams_column};

#[rstest]
#[tokio::test]
async fn can_persist_a_pending_certificate(pending_column: PendingCertificatesColumn) {
    let certificate = Certificate::new(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID, Vec::new()).unwrap();

    assert!(pending_column.insert(&0, &certificate).is_ok());
    assert_eq!(pending_column.get(&0).unwrap(), certificate);
}

#[rstest]
#[tokio::test]
async fn can_persist_a_delivered_certificate(certificates_column: CertificatesColumn) {
    let certificate = Certificate::new(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID, Vec::new()).unwrap();

    assert!(certificates_column
        .insert(&certificate.id, &certificate)
        .is_ok());
    assert_eq!(
        certificates_column.get(&certificate.id).unwrap(),
        certificate
    );
}

#[rstest]
#[tokio::test]
async fn delivered_certificate_position_are_incremented(
    certificates_column: CertificatesColumn,
    source_streams_column: SourceStreamsColumn,
) {
    let certificate = Certificate::new(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID, Vec::new()).unwrap();

    assert!(certificates_column
        .insert(&certificate.id, &certificate)
        .is_ok());
    assert!(source_streams_column
        .insert(
            &SourceStreamPosition(SOURCE_STORAGE_SUBNET_ID, Position::ZERO),
            &certificate.id
        )
        .is_ok());
}

#[rstest]
#[tokio::test]
async fn position_can_be_fetch_for_one_subnet(source_streams_column: SourceStreamsColumn) {
    let certificate = Certificate::new(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID, Vec::new()).unwrap();

    assert!(source_streams_column
        .insert(
            &SourceStreamPosition(SOURCE_STORAGE_SUBNET_ID, Position::ZERO),
            &certificate.id
        )
        .is_ok());

    assert!(matches!(
        source_streams_column
            .prefix_iter(&SOURCE_SUBNET_ID)
            .unwrap()
            .last(),
        Some((SourceStreamPosition(_, Position::ZERO), _))
    ));

    let certificate = Certificate::new(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID, Vec::new()).unwrap();

    assert!(source_streams_column
        .insert(
            &SourceStreamPosition(SOURCE_STORAGE_SUBNET_ID, Position(1)),
            &certificate.id
        )
        .is_ok());

    assert!(matches!(
        source_streams_column
            .prefix_iter(&SOURCE_SUBNET_ID)
            .unwrap()
            .last(),
        Some((SourceStreamPosition(_, Position(1)), _))
    ));
}

#[tokio::test]
#[ignore = "not yet implemented"]
async fn position_can_be_fetch_for_multiple_subnets() {}

#[tokio::test]
#[ignore = "not yet implemented"]
async fn position_can_be_fetch_for_all_subnets() {}
