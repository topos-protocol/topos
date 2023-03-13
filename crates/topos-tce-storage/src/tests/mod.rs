use rstest::rstest;
use test_log::test;
use topos_core::uci::{Certificate, SubnetId};
use tracing::debug;

use crate::{
    rocks::{map::Map, TargetStreamPositionKey},
    Position, RocksDBStorage, Storage,
};

use self::support::storage;

use topos_test_sdk::certificates::create_certificate_chain;
use topos_test_sdk::constants::*;

mod db_columns;
mod position;
mod rocks;
pub(crate) mod support;

const SOURCE_STORAGE_SUBNET_ID: SubnetId = SOURCE_SUBNET_ID_1;
const TARGET_STORAGE_SUBNET_ID_1: SubnetId = TARGET_SUBNET_ID_1;
const TARGET_STORAGE_SUBNET_ID_2: SubnetId = TARGET_SUBNET_ID_2;

#[rstest]
#[test(tokio::test)]
async fn can_persist_a_pending_certificate(storage: RocksDBStorage) {
    let certificate = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[],
        0,
        Vec::new(),
    )
    .unwrap();

    assert!(storage.add_pending_certificate(&certificate).await.is_ok());
}

#[rstest]
#[test(tokio::test)]
async fn can_persist_a_delivered_certificate(storage: RocksDBStorage) {
    let certificates_column = storage.certificates_column();
    let source_streams_column = storage.source_streams_column();
    let target_streams_column = storage.target_streams_column();

    let certificate = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1],
        0,
        Vec::new(),
    )
    .unwrap();

    let cert_id = certificate.id;
    storage.persist(&certificate, None).await.unwrap();

    assert!(certificates_column.get(&cert_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&SOURCE_SUBNET_ID_1)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .1, Position::ZERO);

    let stream_element = target_streams_column
        .prefix_iter::<(SubnetId, SubnetId)>(&(
            TARGET_STORAGE_SUBNET_ID_1,
            SOURCE_STORAGE_SUBNET_ID,
        ))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Position::ZERO);
}

#[rstest]
#[test(tokio::test)]
async fn delivered_certificate_are_added_to_target_stream(storage: RocksDBStorage) {
    let certificates_column = storage.certificates_column();
    let source_streams_column = storage.source_streams_column();
    let target_streams_column = storage.target_streams_column();

    target_streams_column
        .insert(
            &TargetStreamPositionKey(
                TARGET_STORAGE_SUBNET_ID_1,
                SOURCE_STORAGE_SUBNET_ID,
                Position::ZERO,
            ),
            &CERTIFICATE_ID_1,
        )
        .unwrap();

    let certificate = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1, TARGET_SUBNET_ID_2],
        0,
        Vec::new(),
    )
    .unwrap();

    let cert_id = certificate.id;
    storage.persist(&certificate, None).await.unwrap();

    assert!(certificates_column.get(&cert_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&SOURCE_SUBNET_ID_1)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .1, Position::ZERO);

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_STORAGE_SUBNET_ID_1, &SOURCE_STORAGE_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Position(1));

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_STORAGE_SUBNET_ID_2, &SOURCE_STORAGE_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0 .2, Position::ZERO);
}

#[rstest]
#[test(tokio::test)]
async fn pending_certificate_are_removed_during_persist_action(storage: RocksDBStorage) {
    let pending_column = storage.pending_certificates_column();

    let certificate = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1],
        0,
        Vec::new(),
    )
    .unwrap();

    let pending_id = storage.add_pending_certificate(&certificate).await.unwrap();

    assert!(pending_column.get(&pending_id).is_ok());
    storage
        .persist(&certificate, Some(pending_id))
        .await
        .unwrap();

    assert!(pending_column.get(&pending_id).is_err());
}

#[rstest]
#[test(tokio::test)]
async fn fetch_certificates_for_subnets(storage: RocksDBStorage) {
    let other_certificate = Certificate::new(
        PREV_CERTIFICATE_ID,
        TARGET_SUBNET_ID_2,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1],
        0,
        Vec::new(),
    )
    .unwrap();

    storage.persist(&other_certificate, None).await.unwrap();
    let mut expected_certificates =
        create_certificate_chain(SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1, 10);

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
            TARGET_STORAGE_SUBNET_ID_1,
            SOURCE_STORAGE_SUBNET_ID,
            Position::ZERO,
            100,
        )
        .await
        .unwrap();

    certificate_ids.extend(
        storage
            .get_certificates_by_target(
                TARGET_STORAGE_SUBNET_ID_1,
                TARGET_STORAGE_SUBNET_ID_2,
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
#[test(tokio::test)]
async fn pending_certificate_can_be_removed(storage: RocksDBStorage) {
    let pending_column = storage.pending_certificates_column();

    let certificate = Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1],
        0,
        Vec::new(),
    )
    .unwrap();

    let pending_id = storage.add_pending_certificate(&certificate).await.unwrap();

    assert!(pending_column.get(&pending_id).is_ok());
    storage
        .remove_pending_certificate(pending_id)
        .await
        .unwrap();

    assert!(pending_column.get(&pending_id).is_err());

    storage.remove_pending_certificate(1234).await.unwrap();

    assert!(pending_column
        .iter()
        .unwrap()
        .collect::<Vec<_>>()
        .is_empty());

    let _ = storage.add_pending_certificate(&certificate).await.unwrap();

    let pending_id = storage.add_pending_certificate(&certificate).await.unwrap();

    assert!(pending_column.get(&pending_id).is_ok());
    storage
        .remove_pending_certificate(pending_id)
        .await
        .unwrap();

    assert!(!pending_column
        .iter()
        .unwrap()
        .collect::<Vec<_>>()
        .is_empty());
}

#[rstest]
#[test(tokio::test)]
async fn get_source_head_for_subnet(storage: RocksDBStorage) {
    let expected_certificates_for_source_subnet =
        create_certificate_chain(SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_2, 10);

    for cert in &expected_certificates_for_source_subnet {
        storage.persist(cert, None).await.unwrap();
    }

    let expected_certificates_for_source_subnet_a =
        create_certificate_chain(SOURCE_SUBNET_ID_2, TARGET_SUBNET_ID_2, 10);

    for cert in &expected_certificates_for_source_subnet_a {
        storage.persist(cert, None).await.unwrap();
    }

    let last_certificate_subnet_1 = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_1])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    let last_certificate_subnet_2 = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_2])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();

    assert_eq!(
        expected_certificates_for_source_subnet.last().unwrap().id,
        last_certificate_subnet_1.cert_id
    );
    assert_eq!(9, last_certificate_subnet_1.position.unwrap().0); //check position
    assert_eq!(
        expected_certificates_for_source_subnet_a.last().unwrap().id,
        last_certificate_subnet_2.cert_id
    );
    assert_eq!(9, last_certificate_subnet_2.position.unwrap().0); //check position

    let other_certificate = Certificate::new(
        expected_certificates_for_source_subnet.last().unwrap().id,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1],
        0,
        Vec::new(),
    )
    .unwrap();
    storage.persist(&other_certificate, None).await.unwrap();

    let last_certificate_subnet = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_1])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(other_certificate.id, last_certificate_subnet.cert_id);
    assert_eq!(10, last_certificate_subnet.position.unwrap().0); //check position

    let other_certificate_2 = Certificate::new(
        other_certificate.id,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1, TARGET_SUBNET_ID_2],
        0,
        Vec::new(),
    )
    .unwrap();
    storage.persist(&other_certificate_2, None).await.unwrap();

    let last_certificate_subnet = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_1])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(other_certificate_2.id, last_certificate_subnet.cert_id);
    assert_eq!(11, last_certificate_subnet.position.unwrap().0); //check position
}

#[rstest]
#[test(tokio::test)]
async fn get_source_head_for_subnet_pending_certificates(storage: RocksDBStorage) {
    let mut delivered_certificates =
        create_certificate_chain(SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_2, 5);
    delivered_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_2,
        TARGET_SUBNET_ID_2,
        5,
    ));
    delivered_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_3,
        TARGET_SUBNET_ID_2,
        5,
    ));
    delivered_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_1,
        TARGET_SUBNET_ID_2,
        5,
    ));
    delivered_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_3,
        TARGET_SUBNET_ID_2,
        5,
    ));
    delivered_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_1,
        TARGET_SUBNET_ID_1,
        5,
    ));
    delivered_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_3,
        TARGET_SUBNET_ID_1,
        5,
    ));

    let mut pending_certificates =
        create_certificate_chain(SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_2, 3);
    pending_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_2,
        TARGET_SUBNET_ID_2,
        3,
    ));
    pending_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_1,
        TARGET_SUBNET_ID_2,
        3,
    ));
    pending_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_2,
        TARGET_SUBNET_ID_2,
        3,
    ));
    pending_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_3,
        TARGET_SUBNET_ID_2,
        3,
    ));
    pending_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_1,
        TARGET_SUBNET_ID_2,
        3,
    ));
    pending_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_1,
        TARGET_SUBNET_ID_1,
        3,
    ));
    pending_certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_2,
        TARGET_SUBNET_ID_1,
        3,
    ));

    for cert in &delivered_certificates {
        storage.persist(cert, None).await.unwrap();
    }

    for cert in &pending_certificates {
        storage.add_pending_certificate(cert).await.unwrap();
    }

    // Find last pending certificate from the list for every subnet
    let mut expected_pending_certificates: (Certificate, Certificate, Certificate) =
        (Default::default(), Default::default(), Default::default());
    for cert in &pending_certificates {
        if cert.source_subnet_id == SOURCE_SUBNET_ID_1 {
            expected_pending_certificates.0 = cert.clone();
        } else if cert.source_subnet_id == SOURCE_SUBNET_ID_2 {
            expected_pending_certificates.1 = cert.clone();
        } else if cert.source_subnet_id == SOURCE_SUBNET_ID_3 {
            expected_pending_certificates.2 = cert.clone();
        }
    }

    // Find last delivered certificate from the list for every subnet
    let mut expected_delivered_certificates: (Certificate, Certificate, Certificate) =
        (Default::default(), Default::default(), Default::default());
    for cert in &delivered_certificates {
        if cert.source_subnet_id == SOURCE_SUBNET_ID_1 {
            expected_delivered_certificates.0 = cert.clone();
        } else if cert.source_subnet_id == SOURCE_SUBNET_ID_2 {
            expected_delivered_certificates.1 = cert.clone();
        } else if cert.source_subnet_id == SOURCE_SUBNET_ID_3 {
            expected_delivered_certificates.2 = cert.clone();
        }
    }

    debug!("pending certificates:");
    for cert in &pending_certificates {
        debug!("{}", cert.id);
    }

    debug!(
        "expected pending certificates: ({}, {}, {})",
        expected_pending_certificates.0.id,
        expected_pending_certificates.1.id,
        expected_pending_certificates.2.id,
    );

    debug!("delivered certificates:");
    for cert in &delivered_certificates {
        debug!("{}", cert.id);
    }

    debug!(
        "expected delivered certificates: ({}, {}, {})",
        expected_delivered_certificates.0.id,
        expected_delivered_certificates.1.id,
        expected_delivered_certificates.2.id,
    );

    let last_certificate_subnet_1 = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_1])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        expected_pending_certificates.0.id,
        last_certificate_subnet_1.cert_id
    );

    let last_certificate_subnet_2 = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_2])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        expected_pending_certificates.1.id,
        last_certificate_subnet_2.cert_id
    );

    let last_certificate_subnet_3 = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_3])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        expected_pending_certificates.2.id,
        last_certificate_subnet_3.cert_id
    );

    // Persist all pending certificates and check again
    for cert in &pending_certificates {
        let (pending_certificate_id, _) = storage.get_pending_certificate(cert.id).await.unwrap();
        storage
            .persist(cert, Some(pending_certificate_id))
            .await
            .unwrap();
    }

    // Now all last pending certificates should be last delivered certificates
    let last_certificate_subnet_1 = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_1])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        expected_pending_certificates.0.id,
        last_certificate_subnet_1.cert_id
    );

    let last_certificate_subnet_2 = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_2])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        expected_pending_certificates.1.id,
        last_certificate_subnet_2.cert_id
    );

    let last_certificate_subnet_3 = storage
        .get_source_heads(vec![SOURCE_SUBNET_ID_3])
        .await
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(
        expected_pending_certificates.2.id,
        last_certificate_subnet_3.cert_id
    );
}
