use rstest::rstest;
use topos_core::uci::{Amount, Certificate, CrossChainTransaction};

use crate::{
    rocks::{
        map::Map, CertificatesColumn, PendingCertificatesColumn, SourceSubnetStreamsColumn,
        TargetStreamRef, TargetSubnetStreamsColumn,
    },
    tests::support::{INITIAL_SUBNET_ID, TARGET_SUBNET_ID_A, TARGET_SUBNET_ID_B},
    Height, RocksDBStorage, Storage, SubnetId,
};

use self::support::{
    columns::{certificates_column, pending_column, source_streams_column, target_streams_column},
    storage,
};

mod db_columns;
mod height;
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
async fn can_persist_a_delivered_certificate(
    storage: RocksDBStorage,
    certificates_column: CertificatesColumn,
    source_streams_column: SourceSubnetStreamsColumn,
    target_streams_column: TargetSubnetStreamsColumn,
) {
    let certificate = Certificate::new(
        "".into(),
        INITIAL_SUBNET_ID.to_string(),
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
    storage.persist(certificate, None).await.unwrap();

    assert!(certificates_column.get(&cert_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&INITIAL_SUBNET_ID)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .1, Height::ZERO);

    let stream_element = target_streams_column
        .prefix_iter::<(SubnetId, SubnetId)>(&(TARGET_SUBNET_ID_A, INITIAL_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Height::ZERO);
}

#[rstest]
#[tokio::test]
async fn delivered_certificate_are_added_to_target_stream(
    storage: RocksDBStorage,
    certificates_column: CertificatesColumn,
    source_streams_column: SourceSubnetStreamsColumn,
    target_streams_column: TargetSubnetStreamsColumn,
) {
    _ = target_streams_column
        .insert(
            &TargetStreamRef(TARGET_SUBNET_ID_A, INITIAL_SUBNET_ID, Height::ZERO),
            &"certificate_one".to_string(),
        )
        .unwrap();

    let certificate = Certificate::new(
        "".into(),
        INITIAL_SUBNET_ID.to_string(),
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
    storage.persist(certificate, None).await.unwrap();

    assert!(certificates_column.get(&cert_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&INITIAL_SUBNET_ID)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .1, Height::ZERO);

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_SUBNET_ID_A, &INITIAL_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Height(1));

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_SUBNET_ID_B, &INITIAL_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Height::ZERO);
}

#[rstest]
#[tokio::test]
async fn pending_certificate_are_removed_during_persist_action(
    storage: RocksDBStorage,
    pending_column: PendingCertificatesColumn,
) {
    let certificate = Certificate::new(
        "".into(),
        INITIAL_SUBNET_ID.to_string(),
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
        .persist(certificate, Some(pending_id))
        .await
        .unwrap();

    assert!(pending_column.get(&pending_id).is_err());
}
