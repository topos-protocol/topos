use rstest::rstest;
use topos_core::uci::Certificate;

use crate::{
    rocks::{
        map::Map, CertificatesColumn, PendingCertificatesColumn, SourceStreamPosition,
        SourceStreamsColumn,
    },
    tests::support::SOURCE_SUBNET_ID,
    Position,
};

use super::support::columns::{certificates_column, pending_column, source_streams_column};

#[rstest]
#[tokio::test]
async fn can_persist_a_pending_certificate(pending_column: PendingCertificatesColumn) {
    let certificate = Certificate::new("".into(), "source_subnet_id".into(), Vec::new());

    assert!(pending_column.insert(&0, &certificate).is_ok());
    assert_eq!(pending_column.get(&0).unwrap(), certificate);
}

#[rstest]
#[tokio::test]
async fn can_persist_a_delivered_certificate(certificates_column: CertificatesColumn) {
    let certificate = Certificate::new("".into(), "source_subnet_id".into(), Vec::new());

    assert!(certificates_column
        .insert(&certificate.cert_id, &certificate)
        .is_ok());
    assert_eq!(
        certificates_column.get(&certificate.cert_id).unwrap(),
        certificate
    );
}

#[rstest]
#[tokio::test]
async fn delivered_certificate_position_are_incremented(
    certificates_column: CertificatesColumn,
    source_streams_column: SourceStreamsColumn,
) {
    let certificate = Certificate::new("".into(), "source_subnet_id".into(), Vec::new());

    assert!(certificates_column
        .insert(&certificate.cert_id, &certificate)
        .is_ok());
    assert!(source_streams_column
        .insert(
            &SourceStreamPosition(SOURCE_SUBNET_ID, Position::ZERO),
            &certificate.cert_id
        )
        .is_ok());
}

#[rstest]
#[tokio::test]
async fn position_can_be_fetch_for_one_subnet(source_streams_column: SourceStreamsColumn) {
    let certificate = Certificate::new("".into(), "source_subnet_id".into(), Vec::new());

    assert!(source_streams_column
        .insert(
            &SourceStreamPosition(SOURCE_SUBNET_ID, Position::ZERO),
            &certificate.cert_id
        )
        .is_ok());

    assert!(matches!(
        source_streams_column
            .prefix_iter(&SOURCE_SUBNET_ID)
            .unwrap()
            .last(),
        Some((SourceStreamPosition(_, Position::ZERO), _))
    ));

    let certificate = Certificate::new("".into(), "source_subnet_id".into(), Vec::new());

    assert!(source_streams_column
        .insert(
            &SourceStreamPosition(SOURCE_SUBNET_ID, Position(1)),
            &certificate.cert_id
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
