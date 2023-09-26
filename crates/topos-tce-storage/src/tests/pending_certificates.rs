use std::sync::Arc;

use rstest::rstest;
use topos_core::uci::{Certificate, INITIAL_CERTIFICATE_ID};
use topos_test_sdk::{
    certificates::create_certificate_at_position,
    constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1},
};

use super::support::store;
use crate::{store::WriteStore, validator::ValidatorStore};

#[rstest]
fn adding_genesis_pending_certificate(store: Arc<ValidatorStore>) {
    let certificate = Certificate::new_with_default_fields(
        INITIAL_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
    )
    .unwrap();

    let pending_id = store
        .insert_pending_certificate(&certificate)
        .unwrap()
        .unwrap();

    assert_eq!(
        store.get_pending_certificate(&pending_id).unwrap().unwrap(),
        certificate
    );

    assert_eq!(
        store.get_pending_id(&certificate.id).unwrap().unwrap(),
        pending_id
    );
}

#[rstest]
#[tokio::test]
async fn adding_pending_certificate_with_precedence_check_fail(store: Arc<ValidatorStore>) {
    let initial_certificate_delivered = create_certificate_at_position::default();

    let certificate = Certificate::new_with_default_fields(
        initial_certificate_delivered.certificate.id,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
    )
    .unwrap();

    assert!(store
        .insert_pending_certificate(&certificate)
        .unwrap()
        .is_none());

    assert!(store.get_pending_id(&certificate.id).unwrap().is_none());
    assert!(store
        .pending_tables
        .precedence_pool
        .get(&certificate.prev_id)
        .unwrap()
        .is_some());
    store
        .insert_certificate_delivered(&initial_certificate_delivered)
        .await
        .unwrap();

    let pending_id = store.get_pending_id(&certificate.id).unwrap().unwrap();

    assert_eq!(
        store.get_pending_certificate(&pending_id).unwrap().unwrap(),
        certificate
    );
}

#[rstest]
#[tokio::test]
async fn adding_pending_certificate_already_delivered(store: Arc<ValidatorStore>) {
    let initial_certificate_delivered = create_certificate_at_position::default();

    store
        .insert_certificate_delivered(&initial_certificate_delivered)
        .await
        .unwrap();

    assert!(store
        .insert_pending_certificate(&initial_certificate_delivered.certificate)
        .is_err());
}
