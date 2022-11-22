use rstest::rstest;
use topos_core::uci::{Amount, Certificate, CrossChainTransaction};

use crate::{
    rocks::{map::Map, TargetStreamPosition},
    tests::support::{SOURCE_SUBNET_ID, TARGET_SUBNET_ID_A, TARGET_SUBNET_ID_B},
    Position, RocksDBStorage, Storage, SubnetId,
};

use self::support::storage;

mod db_columns;
mod position;
mod rocks;
pub(crate) mod support;

#[rstest]
#[tokio::test]
async fn can_persist_a_pending_certificate(storage: RocksDBStorage) {
    let certificate = Certificate::new("cert_id".into(), "source_subnet_id".into(), Vec::new());

    assert!(storage.add_pending_certificate(certificate).await.is_ok());
}

#[rstest]
#[tokio::test]
async fn can_persist_a_delivered_certificate(storage: RocksDBStorage) {
    let certificates_column = storage.certificates_column();
    let source_streams_column = storage.source_streams_column();
    let target_streams_column = storage.target_streams_column();

    let certificate = Certificate::new(
        "".into(),
        SOURCE_SUBNET_ID.to_string(),
        vec![CrossChainTransaction {
            recipient_addr: "".into(),
            sender_addr: "source_subnet_a".into(),
            terminal_subnet_id: TARGET_SUBNET_ID_A.to_string(),
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                asset_id: "asset_id".into(),
                amount: Amount::from(1),
            },
        }],
    );

    let cert_id = certificate.cert_id.clone();
    storage.persist(&certificate, None).await.unwrap();

    assert!(certificates_column.get(&cert_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&SOURCE_SUBNET_ID)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .1, Position::ZERO);

    let stream_element = target_streams_column
        .prefix_iter::<(SubnetId, SubnetId)>(&(TARGET_SUBNET_ID_A, SOURCE_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Position::ZERO);
}

#[rstest]
#[tokio::test]
async fn delivered_certificate_are_added_to_target_stream(storage: RocksDBStorage) {
    let certificates_column = storage.certificates_column();
    let source_streams_column = storage.source_streams_column();
    let target_streams_column = storage.target_streams_column();

    _ = target_streams_column
        .insert(
            &TargetStreamPosition(TARGET_SUBNET_ID_A, SOURCE_SUBNET_ID, Position::ZERO),
            &"certificate_one".to_string(),
        )
        .unwrap();

    let certificate = Certificate::new(
        "".into(),
        SOURCE_SUBNET_ID.to_string(),
        vec![
            CrossChainTransaction {
                recipient_addr: "".into(),
                sender_addr: "source_subnet_a".into(),
                terminal_subnet_id: TARGET_SUBNET_ID_A.to_string(),
                transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                    asset_id: "asset_id".into(),
                    amount: Amount::from(1),
                },
            },
            CrossChainTransaction {
                recipient_addr: "".into(),
                sender_addr: "source_subnet_a".into(),
                terminal_subnet_id: TARGET_SUBNET_ID_B.to_string(),
                transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                    asset_id: "asset_id".into(),
                    amount: Amount::from(1),
                },
            },
        ],
    );

    let cert_id = certificate.cert_id.clone();
    storage.persist(&certificate, None).await.unwrap();

    assert!(certificates_column.get(&cert_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&SOURCE_SUBNET_ID)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .1, Position::ZERO);

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_SUBNET_ID_A, &SOURCE_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Position(1));

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_SUBNET_ID_B, &SOURCE_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Position::ZERO);
}

#[rstest]
#[tokio::test]
async fn pending_certificate_are_removed_during_persist_action(storage: RocksDBStorage) {
    let pending_column = storage.pending_certificates_column();

    let certificate = Certificate::new(
        "".into(),
        SOURCE_SUBNET_ID.to_string(),
        vec![CrossChainTransaction {
            recipient_addr: "".into(),
            sender_addr: "source_subnet_a".into(),
            terminal_subnet_id: TARGET_SUBNET_ID_A.to_string(),
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                asset_id: "asset_id".into(),
                amount: Amount::from(1),
            },
        }],
    );

    let pending_id = storage
        .add_pending_certificate(certificate.clone())
        .await
        .unwrap();

    assert!(pending_column.get(&pending_id).is_ok());
    _ = storage
        .persist(&certificate, Some(pending_id))
        .await
        .unwrap();

    assert!(pending_column.get(&pending_id).is_err());
}

#[rstest]
#[tokio::test]
async fn fetch_certificates_for_subnets(storage: RocksDBStorage) {
    let other_certificate = Certificate::new(
        "".into(),
        TARGET_SUBNET_ID_B.to_string(),
        vec![CrossChainTransaction {
            recipient_addr: "".into(),
            sender_addr: "source_subnet_a".into(),
            terminal_subnet_id: TARGET_SUBNET_ID_A.to_string(),
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                asset_id: "asset_id".into(),
                amount: Amount::from(1),
            },
        }],
    );

    storage.persist(&other_certificate, None).await.unwrap();
    let mut expected_certificates = create_certificate_chain(
        SOURCE_SUBNET_ID.to_string(),
        TARGET_SUBNET_ID_A.to_string(),
        10,
    );

    for cert in &expected_certificates {
        storage.persist(cert, None).await.unwrap();
    }

    let mut certificate_ids = storage
        .get_certificates_by_source(SOURCE_SUBNET_ID, Position::ZERO, 5)
        .await
        .unwrap();

    assert_eq!(5, certificate_ids.len());

    let certificate_ids_second = storage
        .get_certificates_by_source(SOURCE_SUBNET_ID, Position(5), 5)
        .await
        .unwrap();

    assert_eq!(5, certificate_ids_second.len());

    certificate_ids.extend(certificate_ids_second.into_iter());

    let certificates = storage.get_certificates(certificate_ids).await.unwrap();
    assert_eq!(expected_certificates, certificates);

    let mut certificate_ids = storage
        .get_certificates_by_target(TARGET_SUBNET_ID_A, SOURCE_SUBNET_ID, Position::ZERO, 100)
        .await
        .unwrap();

    certificate_ids.extend(
        storage
            .get_certificates_by_target(TARGET_SUBNET_ID_A, TARGET_SUBNET_ID_B, Position::ZERO, 100)
            .await
            .unwrap()
            .into_iter(),
    );

    assert_eq!(11, certificate_ids.len());

    let certificates = storage.get_certificates(certificate_ids).await.unwrap();
    expected_certificates.push(other_certificate);
    assert_eq!(expected_certificates, certificates);
}

#[rstest]
#[tokio::test]
async fn pending_certificate_can_be_removed(storage: RocksDBStorage) {
    let pending_column = storage.pending_certificates_column();

    let certificate = Certificate::new(
        "".into(),
        SOURCE_SUBNET_ID.to_string(),
        vec![CrossChainTransaction {
            recipient_addr: "".into(),
            sender_addr: "source_subnet_a".into(),
            terminal_subnet_id: TARGET_SUBNET_ID_A.to_string(),
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                asset_id: "asset_id".into(),
                amount: Amount::from(1),
            },
        }],
    );

    let pending_id = storage
        .add_pending_certificate(certificate.clone())
        .await
        .unwrap();

    assert!(pending_column.get(&pending_id).is_ok());
    _ = storage
        .remove_pending_certificate(pending_id)
        .await
        .unwrap();

    assert!(pending_column.get(&pending_id).is_err());

    _ = storage.remove_pending_certificate(1234).await.unwrap();

    assert!(pending_column
        .iter()
        .unwrap()
        .collect::<Vec<_>>()
        .is_empty());

    let _ = storage
        .add_pending_certificate(certificate.clone())
        .await
        .unwrap();

    let pending_id = storage
        .add_pending_certificate(certificate.clone())
        .await
        .unwrap();

    assert!(pending_column.get(&pending_id).is_ok());
    _ = storage
        .remove_pending_certificate(pending_id)
        .await
        .unwrap();

    assert!(!pending_column
        .iter()
        .unwrap()
        .collect::<Vec<_>>()
        .is_empty());
}

fn create_certificate_chain(
    source_subnet: String,
    target_subnet: String,
    number: usize,
) -> Vec<Certificate> {
    let mut certificates = Vec::new();
    let mut parent = None;

    for _ in 0..number {
        let cert = Certificate::new(
            parent.take().unwrap_or(String::new()),
            source_subnet.clone(),
            vec![CrossChainTransaction {
                recipient_addr: target_subnet.clone(),
                sender_addr: source_subnet.clone(),
                terminal_subnet_id: target_subnet.clone(),
                transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                    asset_id: "asset_id".into(),
                    amount: Amount::from(1),
                },
            }],
        );
        parent = Some(cert.cert_id.clone());
        certificates.push(cert);
    }

    certificates
}
