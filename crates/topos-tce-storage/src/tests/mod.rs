use crate::SubnetId;
use rstest::rstest;
use topos_core::uci::{Amount, Certificate, CrossChainTransaction};

use crate::{
    rocks::{map::Map, TargetStreamPosition},
    Position, RocksDBStorage, Storage,
};

use self::support::storage;

mod db_columns;
mod position;
mod rocks;
pub(crate) mod support;

const SOURCE_SUBNET_ID: topos_core::uci::SubnetId = [1u8; 32];
const TARGET_SUBNET_ID_A: topos_core::uci::SubnetId = [2u8; 32];
const TARGET_SUBNET_ID_B: topos_core::uci::SubnetId = [3u8; 32];

const PREV_CERTIFICATE_ID: topos_core::uci::CertificateId = [0u8; 32];
const CERTIFICATE_ID: topos_core::uci::CertificateId = [5u8; 32];
const SENDER_ID: topos_core::uci::Address = [6u8; 20];
const RECEIVER_ID_A: topos_core::uci::Address = [7u8; 20];
const RECEIVER_ID_B: topos_core::uci::Address = [8u8; 20];

const SOURCE_STORAGE_SUBNET_ID: SubnetId = SubnetId { inner: [1u8; 32] };
const TARGET_STORAGE_SUBNET_ID_A: SubnetId = SubnetId { inner: [2u8; 32] };
const TARGET_STORAGE_SUBNET_ID_B: SubnetId = SubnetId { inner: [3u8; 32] };

#[rstest]
#[tokio::test]
async fn can_persist_a_pending_certificate(storage: RocksDBStorage) {
    let certificate = Certificate::new(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID, Vec::new()).unwrap();

    assert!(storage.add_pending_certificate(certificate).await.is_ok());
}

#[rstest]
#[tokio::test]
async fn can_persist_a_delivered_certificate(storage: RocksDBStorage) {
    let certificates_column = storage.certificates_column();
    let source_streams_column = storage.source_streams_column();
    let target_streams_column = storage.target_streams_column();

    let certificate = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID,
        vec![CrossChainTransaction {
            target_subnet_id: TARGET_SUBNET_ID_A,
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                sender: SENDER_ID,
                receiver: RECEIVER_ID_A,
                symbol: "asset_id".to_string(),
                amount: Amount::from(1),
            },
        }],
    )
    .unwrap();

    let cert_id = certificate.id.clone();
    storage.persist(&certificate, None).await.unwrap();

    assert!(certificates_column.get(&cert_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&SOURCE_SUBNET_ID)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .1, Position::ZERO);

    let stream_element = target_streams_column
        .prefix_iter::<(SubnetId, SubnetId)>(&(
            TARGET_STORAGE_SUBNET_ID_A,
            SOURCE_STORAGE_SUBNET_ID,
        ))
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
            &TargetStreamPosition(
                TARGET_STORAGE_SUBNET_ID_A,
                SOURCE_STORAGE_SUBNET_ID,
                Position::ZERO,
            ),
            &CERTIFICATE_ID,
        )
        .unwrap();

    let certificate = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID,
        vec![
            CrossChainTransaction {
                target_subnet_id: TARGET_SUBNET_ID_A,
                transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                    sender: SENDER_ID,
                    receiver: RECEIVER_ID_A,
                    symbol: "asset_id".to_string(),
                    amount: Amount::from(1),
                },
            },
            CrossChainTransaction {
                target_subnet_id: TARGET_SUBNET_ID_B,
                transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                    sender: SENDER_ID,
                    receiver: RECEIVER_ID_B,
                    symbol: "asset_id".to_string(),
                    amount: Amount::from(1),
                },
            },
        ],
    )
    .unwrap();

    let cert_id = certificate.id.clone();
    storage.persist(&certificate, None).await.unwrap();

    assert!(certificates_column.get(&cert_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&SOURCE_SUBNET_ID)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .1, Position::ZERO);

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_STORAGE_SUBNET_ID_A, &SOURCE_STORAGE_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Position(1));

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_STORAGE_SUBNET_ID_B, &SOURCE_STORAGE_SUBNET_ID))
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
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID,
        vec![CrossChainTransaction {
            target_subnet_id: TARGET_SUBNET_ID_A,
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                sender: SENDER_ID,
                receiver: RECEIVER_ID_A,
                symbol: "asset_id".into(),
                amount: Amount::from(1),
            },
        }],
    )
    .unwrap();

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
        PREV_CERTIFICATE_ID,
        TARGET_SUBNET_ID_B,
        vec![CrossChainTransaction {
            target_subnet_id: TARGET_SUBNET_ID_A,
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                sender: SENDER_ID,
                receiver: RECEIVER_ID_A,
                symbol: "asset_id".into(),
                amount: Amount::from(1),
            },
        }],
    )
    .unwrap();

    storage.persist(&other_certificate, None).await.unwrap();
    let mut expected_certificates =
        create_certificate_chain(SOURCE_SUBNET_ID, TARGET_SUBNET_ID_A, 10);

    for cert in &expected_certificates {
        storage.persist(cert, None).await.unwrap();
    }

    let mut certificate_ids = storage
        .get_certificates_by_source(SOURCE_STORAGE_SUBNET_ID, Position::ZERO, 5)
        .await
        .unwrap();

    assert_eq!(5, certificate_ids.len());

    let certificate_ids_second = storage
        .get_certificates_by_source(SOURCE_STORAGE_SUBNET_ID, Position(5), 5)
        .await
        .unwrap();

    assert_eq!(5, certificate_ids_second.len());

    certificate_ids.extend(certificate_ids_second.into_iter());

    let certificates = storage.get_certificates(certificate_ids).await.unwrap();
    assert_eq!(expected_certificates, certificates);

    let mut certificate_ids = storage
        .get_certificates_by_target(
            TARGET_STORAGE_SUBNET_ID_A,
            SOURCE_STORAGE_SUBNET_ID,
            Position::ZERO,
            100,
        )
        .await
        .unwrap();

    certificate_ids.extend(
        storage
            .get_certificates_by_target(
                TARGET_STORAGE_SUBNET_ID_A,
                TARGET_STORAGE_SUBNET_ID_B,
                Position::ZERO,
                100,
            )
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
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID,
        vec![CrossChainTransaction {
            target_subnet_id: TARGET_SUBNET_ID_A,
            transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                sender: SENDER_ID,
                receiver: RECEIVER_ID_A,
                symbol: "asset_id".into(),
                amount: Amount::from(1),
            },
        }],
    )
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
    source_subnet: topos_core::uci::SubnetId,
    target_subnet: topos_core::uci::SubnetId,
    number: usize,
) -> Vec<Certificate> {
    let mut certificates = Vec::new();
    let mut parent = None;

    for _ in 0..number {
        let cert = Certificate::new(
            parent.take().unwrap_or([0u8; 32]),
            source_subnet.clone(),
            vec![CrossChainTransaction {
                target_subnet_id: target_subnet.clone(),
                transaction_data: topos_core::uci::CrossChainTransactionData::AssetTransfer {
                    sender: SENDER_ID,
                    receiver: RECEIVER_ID_A,
                    symbol: "asset_id".into(),
                    amount: Amount::from(1),
                },
            }],
        )
        .unwrap();
        parent = Some(cert.id.clone());
        certificates.push(cert);
    }

    certificates
}
