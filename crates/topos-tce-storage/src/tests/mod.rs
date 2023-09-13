use rstest::rstest;
use std::sync::Arc;
use test_log::test;
use topos_core::{
    types::{
        stream::{CertificateSourceStreamPosition, CertificateTargetStreamPosition, Position},
        CertificateDelivered, ProofOfDelivery,
    },
    uci::{Certificate, SubnetId},
};

use crate::{
    rocks::map::Map,
    store::{ReadStore, WriteStore},
    validator::ValidatorStore,
};

use self::support::store;

use topos_test_sdk::certificates::create_certificate_chain;
use topos_test_sdk::constants::*;

mod db_columns;
mod pending_certificates;
mod position;
mod rocks;
pub(crate) mod support;

const SOURCE_STORAGE_SUBNET_ID: SubnetId = SOURCE_SUBNET_ID_1;
const TARGET_STORAGE_SUBNET_ID_1: SubnetId = TARGET_SUBNET_ID_1;
const TARGET_STORAGE_SUBNET_ID_2: SubnetId = TARGET_SUBNET_ID_2;

#[rstest]
#[test]
fn can_persist_a_pending_certificate(store: Arc<ValidatorStore>) {
    let certificate =
        Certificate::new_with_default_fields(PREV_CERTIFICATE_ID, SOURCE_SUBNET_ID_1, &[]).unwrap();

    assert!(store.insert_pending_certificate(&certificate).is_ok());
}

#[rstest]
#[test(tokio::test)]
async fn can_persist_a_delivered_certificate(store: Arc<ValidatorStore>) {
    let certificate = Certificate::new_with_default_fields(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
    )
    .unwrap();
    let certificate_id = certificate.id;
    let certificate = CertificateDelivered {
        certificate,
        proof_of_delivery: ProofOfDelivery {
            delivery_position: CertificateSourceStreamPosition::new(
                SOURCE_SUBNET_ID_1,
                Position::ZERO,
            ),
            readies: vec![],
            threshold: 0,
            certificate_id,
        },
    };

    store
        .insert_certificate_delivered(&certificate)
        .await
        .unwrap();

    let certificates_table = store.fullnode_store.perpetual_tables.certificates.clone();
    let streams_table = store.fullnode_store.perpetual_tables.streams.clone();
    let targets_streams_table = store.fullnode_store.index_tables.target_streams.clone();

    assert!(certificates_table.get(&certificate.certificate.id).is_ok());

    let stream_element = streams_table
        .prefix_iter(&SOURCE_SUBNET_ID_1)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0.position, Position::ZERO);

    let stream_element = targets_streams_table
        .prefix_iter::<(SubnetId, SubnetId)>(&(
            TARGET_STORAGE_SUBNET_ID_1,
            SOURCE_STORAGE_SUBNET_ID,
        ))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0.position, Position::ZERO);
    assert_eq!(stream_element.1, certificate.certificate.id);
}

#[rstest]
#[test(tokio::test)]
async fn delivered_certificate_are_added_to_target_stream(store: Arc<ValidatorStore>) {
    let certificates_column = store.fullnode_store.perpetual_tables.certificates.clone();
    let source_streams_column = store.fullnode_store.perpetual_tables.streams.clone();
    let target_streams_column = store.fullnode_store.index_tables.target_streams.clone();

    target_streams_column
        .insert(
            &CertificateTargetStreamPosition::new(
                TARGET_STORAGE_SUBNET_ID_1,
                SOURCE_STORAGE_SUBNET_ID,
                Position::ZERO,
            ),
            &CERTIFICATE_ID_1,
        )
        .unwrap();

    let certificate = Certificate::new_with_default_fields(
        CERTIFICATE_ID_1,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1, TARGET_SUBNET_ID_2],
    )
    .unwrap();

    let certificate_id = certificate.id;
    let certificate = CertificateDelivered {
        certificate,
        proof_of_delivery: ProofOfDelivery {
            delivery_position: CertificateSourceStreamPosition::new(
                SOURCE_SUBNET_ID_1,
                Position::ZERO,
            ),
            readies: vec![],
            threshold: 0,
            certificate_id,
        },
    };
    store
        .insert_certificate_delivered(&certificate)
        .await
        .unwrap();

    assert!(certificates_column.get(&certificate_id).is_ok());

    let stream_element = source_streams_column
        .prefix_iter(&SOURCE_SUBNET_ID_1)
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0.position, Position::ZERO);

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_STORAGE_SUBNET_ID_1, &SOURCE_STORAGE_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(*stream_element.0.position, 1);

    let stream_element = target_streams_column
        .prefix_iter(&(&TARGET_STORAGE_SUBNET_ID_2, &SOURCE_STORAGE_SUBNET_ID))
        .unwrap()
        .last()
        .unwrap();

    assert_eq!(stream_element.0.position, Position::ZERO);
}

#[rstest]
#[test(tokio::test)]
async fn pending_certificate_are_removed_during_persist_action(store: Arc<ValidatorStore>) {
    let pending_column = store.pending_tables.pending_pool.clone();

    let certificate = Certificate::new_with_default_fields(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
    )
    .unwrap();

    let certificate_id = certificate.id;
    let pending_id = store
        .insert_pending_certificate(&certificate)
        .unwrap()
        .unwrap();

    let certificate = CertificateDelivered {
        certificate,
        proof_of_delivery: ProofOfDelivery {
            certificate_id,
            delivery_position: CertificateSourceStreamPosition::new(
                SOURCE_SUBNET_ID_1,
                Position::ZERO,
            ),
            readies: vec![],
            threshold: 0,
        },
    };
    assert!(pending_column.get(&pending_id).is_ok());
    store
        .insert_certificate_delivered(&certificate)
        .await
        .unwrap();

    assert!(matches!(pending_column.get(&pending_id), Ok(None)));
}

#[rstest]
#[test(tokio::test)]
async fn fetch_certificates_for_subnets(store: Arc<ValidatorStore>) {
    let other_certificate = Certificate::new_with_default_fields(
        PREV_CERTIFICATE_ID,
        TARGET_SUBNET_ID_2,
        &[TARGET_SUBNET_ID_1],
    )
    .unwrap();

    let certificate_id = other_certificate.id;
    let other_certificate = CertificateDelivered {
        certificate: other_certificate,
        proof_of_delivery: ProofOfDelivery {
            certificate_id,
            delivery_position: CertificateSourceStreamPosition::new(
                TARGET_SUBNET_ID_2,
                Position::ZERO,
            ),
            readies: vec![],
            threshold: 0,
        },
    };

    store
        .insert_certificate_delivered(&other_certificate)
        .await
        .unwrap();

    let mut expected_certificates =
        create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 10)
            .into_iter()
            .enumerate()
            .map(|(index, v)| CertificateDelivered {
                certificate: v.certificate.clone(),
                proof_of_delivery: ProofOfDelivery {
                    certificate_id: v.certificate.id,
                    delivery_position: CertificateSourceStreamPosition::new(
                        SOURCE_SUBNET_ID_1,
                        index as u64,
                    ),
                    readies: vec![],
                    threshold: 0,
                },
            })
            .collect::<Vec<_>>();

    for cert in &expected_certificates {
        store.insert_certificate_delivered(cert).await.unwrap();
    }

    let mut certificate_ids = store
        .get_source_stream_certificates_from_position(
            CertificateSourceStreamPosition::new(SOURCE_STORAGE_SUBNET_ID, Position::ZERO),
            5,
        )
        .unwrap()
        .into_iter()
        .map(|(certificate, _)| certificate.certificate.id)
        .collect::<Vec<_>>();

    assert_eq!(5, certificate_ids.len());

    let certificate_ids_second = store
        .get_source_stream_certificates_from_position(
            CertificateSourceStreamPosition::new(SOURCE_STORAGE_SUBNET_ID, 5),
            5,
        )
        .unwrap()
        .into_iter()
        .map(|(certificate, _)| certificate.certificate.id)
        .collect::<Vec<_>>();

    assert_eq!(5, certificate_ids_second.len());

    certificate_ids.extend(certificate_ids_second.into_iter());

    let certificates = store
        .get_certificates(&certificate_ids[..])
        .unwrap()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    assert_eq!(expected_certificates, certificates);

    let mut certificate_ids = store
        .get_target_stream_certificates_from_position(
            CertificateTargetStreamPosition::new(
                TARGET_STORAGE_SUBNET_ID_1,
                SOURCE_STORAGE_SUBNET_ID,
                Position::ZERO,
            ),
            100,
        )
        .unwrap()
        .into_iter()
        .map(|(c, _)| c.certificate.id)
        .collect::<Vec<_>>();

    certificate_ids.extend(
        store
            .get_target_stream_certificates_from_position(
                CertificateTargetStreamPosition::new(
                    TARGET_STORAGE_SUBNET_ID_1,
                    TARGET_STORAGE_SUBNET_ID_2,
                    Position::ZERO,
                ),
                100,
            )
            .unwrap()
            .into_iter()
            .map(|(c, _)| c.certificate.id),
    );

    assert_eq!(11, certificate_ids.len());

    let certificates = store
        .get_certificates(&certificate_ids[..])
        .unwrap()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    expected_certificates.push(other_certificate);

    assert_eq!(expected_certificates, certificates);
}

#[rstest]
#[test(tokio::test)]
async fn pending_certificate_can_be_removed(store: Arc<ValidatorStore>) {
    let pending_column = store.pending_tables.pending_pool.clone();

    let certificate = Certificate::new_with_default_fields(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
    )
    .unwrap();

    let pending_id = store
        .insert_pending_certificate(&certificate)
        .unwrap()
        .unwrap();

    assert!(pending_column.get(&pending_id).is_ok());
    store.delete_pending_certificate(&pending_id).unwrap();

    assert!(matches!(pending_column.get(&pending_id), Ok(None)));

    let _ = store.insert_pending_certificate(&certificate).unwrap();

    let pending_id = store
        .insert_pending_certificate(&certificate)
        .unwrap()
        .unwrap();

    assert!(pending_column.get(&pending_id).is_ok());
    store.delete_pending_certificate(&pending_id).unwrap();

    assert!(pending_column.iter().unwrap().next().is_some());
}

#[rstest]
#[test(tokio::test)]
async fn get_source_head_for_subnet(store: Arc<ValidatorStore>) {
    let expected_certificates_for_source_subnet_1: Vec<_> =
        create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_2], 10);

    store
        .insert_certificates_delivered(&expected_certificates_for_source_subnet_1)
        .await
        .unwrap();

    let expected_certificates_for_source_subnet_2 =
        create_certificate_chain(SOURCE_SUBNET_ID_2, &[TARGET_SUBNET_ID_2], 10);

    store
        .insert_certificates_delivered(&expected_certificates_for_source_subnet_2)
        .await
        .unwrap();

    let last_certificate_source_subnet_1 =
        store.get_source_head(&SOURCE_SUBNET_ID_1).unwrap().unwrap();
    let last_certificate_source_subnet_2 =
        store.get_source_head(&SOURCE_SUBNET_ID_2).unwrap().unwrap();

    assert_eq!(
        expected_certificates_for_source_subnet_1
            .last()
            .unwrap()
            .certificate
            .id,
        last_certificate_source_subnet_1.certificate_id
    );
    assert_eq!(9, *last_certificate_source_subnet_1.position); //check position
    assert_eq!(
        expected_certificates_for_source_subnet_2
            .last()
            .unwrap()
            .certificate
            .id,
        last_certificate_source_subnet_2.certificate_id
    );
    assert_eq!(9, *last_certificate_source_subnet_2.position); //check position

    let certificate = Certificate::new_with_default_fields(
        expected_certificates_for_source_subnet_1
            .last()
            .unwrap()
            .certificate
            .id,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
    )
    .unwrap();

    let new_certificate_source_subnet_1 = CertificateDelivered {
        certificate: certificate.clone(),
        proof_of_delivery: ProofOfDelivery {
            certificate_id: certificate.id,
            delivery_position: CertificateSourceStreamPosition::new(SOURCE_SUBNET_ID_1, 10),
            readies: vec![],
            threshold: 0,
        },
    };

    store
        .insert_certificate_delivered(&new_certificate_source_subnet_1)
        .await
        .unwrap();

    let last_certificate_subnet_1 = store.get_source_head(&SOURCE_SUBNET_ID_1).unwrap().unwrap();

    assert_eq!(
        new_certificate_source_subnet_1.certificate.id,
        last_certificate_subnet_1.certificate_id
    );
    assert_eq!(10, *last_certificate_subnet_1.position); //check position

    let other_certificate_2 = Certificate::new_with_default_fields(
        new_certificate_source_subnet_1.certificate.id,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_2, TARGET_SUBNET_ID_1],
    )
    .unwrap();
    let other_certificate_2 = CertificateDelivered {
        certificate: other_certificate_2.clone(),
        proof_of_delivery: ProofOfDelivery {
            certificate_id: other_certificate_2.id,
            delivery_position: CertificateSourceStreamPosition::new(SOURCE_SUBNET_ID_1, 11),
            readies: vec![],
            threshold: 0,
        },
    };

    store
        .insert_certificate_delivered(&other_certificate_2)
        .await
        .unwrap();

    let last_certificate_subnet_2 = store.get_source_head(&SOURCE_SUBNET_ID_1).unwrap().unwrap();
    assert_eq!(
        other_certificate_2.certificate.id,
        last_certificate_subnet_2.certificate_id
    );
    assert_eq!(11, *last_certificate_subnet_2.position); //check position
}

#[rstest]
#[test(tokio::test)]
async fn get_pending_certificates(store: Arc<ValidatorStore>) {
    let certificates_for_source_subnet_1 =
        create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_2], 15);
    let certificates_for_source_subnet_2 =
        create_certificate_chain(SOURCE_SUBNET_ID_2, &[TARGET_SUBNET_ID_2], 15);

    // Persist the first 10 Cert of each Subnets
    store
        .insert_certificates_delivered(&certificates_for_source_subnet_1[..10])
        .await
        .unwrap();
    store
        .insert_certificates_delivered(&certificates_for_source_subnet_2[..10])
        .await
        .unwrap();

    let mut expected_pending_certificates = certificates_for_source_subnet_1[10..]
        .iter()
        .enumerate()
        .map(|(index, certificate)| (index as u64, certificate.certificate.clone()))
        .collect::<Vec<_>>();

    expected_pending_certificates.extend(
        certificates_for_source_subnet_2[10..]
            .iter()
            .enumerate()
            .map(|(index, certificate)| {
                (
                    index as u64 + expected_pending_certificates.len() as u64,
                    certificate.certificate.clone(),
                )
            })
            .collect::<Vec<_>>(),
    );

    // Add the last 5 cert of each Subnet as pending certificate
    store
        .insert_pending_certificates(
            &certificates_for_source_subnet_1[10..]
                .iter()
                .map(|certificate| certificate.certificate.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap();

    store
        .insert_pending_certificates(
            &certificates_for_source_subnet_2[10..]
                .iter()
                .map(|certificate| certificate.certificate.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap();

    let pending_certificates = store.get_pending_certificates().unwrap();
    assert_eq!(
        expected_pending_certificates.len(),
        pending_certificates.len()
    );
    assert_eq!(expected_pending_certificates, pending_certificates);

    // Remove some pending certificates, check again
    let cert_to_remove = expected_pending_certificates.remove(5);
    store.delete_pending_certificate(&cert_to_remove.0).unwrap();

    let cert_to_remove = expected_pending_certificates.remove(8);
    store.delete_pending_certificate(&cert_to_remove.0).unwrap();

    let pending_certificates = store.get_pending_certificates().unwrap();
    assert_eq!(
        expected_pending_certificates.len(),
        pending_certificates.len()
    );
    assert_eq!(expected_pending_certificates, pending_certificates);
}

#[rstest]
#[tokio::test]
async fn fetch_source_subnet_certificates_in_order(store: Arc<ValidatorStore>) {
    let certificates_for_source_subnet_1 =
        create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_2], 10);
    // Persist the first 10 Cert of each Subnets
    store
        .insert_certificates_delivered(&certificates_for_source_subnet_1[..10])
        .await
        .unwrap();

    let res = store
        .get_source_stream_certificates_from_position(
            crate::CertificateSourceStreamPosition {
                subnet_id: SOURCE_SUBNET_ID_1,
                position: Position::ZERO,
            },
            100,
        )
        .unwrap();

    let mut prev = PREV_CERTIFICATE_ID;

    for (index, (cert, position)) in res.iter().enumerate() {
        let cert = &cert.certificate;
        assert_eq!(cert.prev_id, prev);
        assert!(matches!(
            position,
            CertificateSourceStreamPosition {
                subnet_id: SOURCE_SUBNET_ID_1,
                position: current_pos
            } if **current_pos == index as u64
        ));

        prev = cert.id;
    }
}
