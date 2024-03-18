use std::{sync::Arc, time::Duration};

use rstest::rstest;
use topos_core::uci::{Certificate, INITIAL_CERTIFICATE_ID};
use topos_test_sdk::{
    certificates::{create_certificate_at_position, create_certificate_chain},
    constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1},
};

use super::support::store;
use crate::{store::WriteStore, validator::ValidatorStore};

#[rstest]
#[tokio::test]
async fn adding_genesis_pending_certificate(store: Arc<ValidatorStore>) {
    let certificate = Certificate::new_with_default_fields(
        INITIAL_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
    )
    .unwrap();

    let pending_id = store
        .insert_pending_certificate(&certificate)
        .await
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
        .await
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
        .await
        .is_err());
}

/// This test is covering a corner case which involves the delivery of a prev certificate
/// and a child certificate.
///
/// The scenario is this one:
/// - A `prev` certificate (`C1`) has been delivered (by the broadcast) and need to be persisted
///   The persist method will hold a lock while performing multiple insert/delete to avoid
///   insert race condition.
/// - At the same time, another node is sending a certificate (`C2`) which have `C1` as `prev_id`.
///   `C2` is looking at the storage to find if the `prev_id` `C1` is delivered but find nothing as
///   the `persist` method is still working at creating the `WriteBatch`. It led the node to put
///   `C2` in the `precedence_pool` waiting for `C1` to be delivered while it is in fact already
///   delivered.
///
/// To avoid that and as a first step, when trying to insert a certificate in the pending pool,
/// The node will try to acquire a lock guard on the certificate but also on the prev_id.
mod concurrency {
    use crate::errors::StorageError;

    use super::*;

    #[rstest]
    #[tokio::test]
    async fn adding_pending_certificate_but_prev_fail(store: Arc<ValidatorStore>) {
        let mut certs = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 2);
        let cert = certs.pop().unwrap();
        let parent = certs.pop().unwrap();

        assert!(certs.is_empty());

        // The lock guard simulate the start of the certificate insertion in the table.
        let lock_guard_certificate = store
            .fullnode_store
            .certificate_lock_guard(parent.certificate.id)
            .await;

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            // Drop the lock_guard of the prev_id without inserting it
            drop(lock_guard_certificate);
        });

        assert!(matches!(
            store.insert_pending_certificate(&cert.certificate).await,
            Ok(None)
        ));
    }

    #[rstest]
    #[tokio::test]
    async fn certificate_in_delivery(store: Arc<ValidatorStore>) {
        let mut certs = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 1);
        let cert = certs.pop().unwrap();

        assert!(certs.is_empty());

        // The lock guard simulate the start of the certificate insertion in the table.
        let lock_guard_subnet = store
            .fullnode_store
            .subnet_lock_guard(cert.certificate.source_subnet_id)
            .await;

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            // Drop the lock_guard of the certificate without inserting it
            drop(lock_guard_subnet);
        });

        let store_deliver = store.clone();
        let delivered = cert.clone();
        tokio::spawn(async move {
            _ = store_deliver
                .insert_certificate_delivered(&delivered)
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(matches!(
            store.insert_pending_certificate(&cert.certificate).await,
            Err(StorageError::InternalStorage(
                crate::errors::InternalStorageError::CertificateAlreadyExists
            ))
        ));
    }

    #[rstest]
    #[tokio::test]
    async fn prev_certificate_in_delivery(store: Arc<ValidatorStore>) {
        let mut certs = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 2);
        let cert = certs.pop().unwrap();
        let prev = certs.pop().unwrap();

        assert!(certs.is_empty());

        // The lock guard simulate the start of the certificate insertion in the table.
        let lock_guard_subnet = store
            .fullnode_store
            .subnet_lock_guard(cert.certificate.source_subnet_id)
            .await;

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            // Drop the lock_guard of the certificate without inserting it
            drop(lock_guard_subnet);
        });

        let store_deliver = store.clone();
        tokio::spawn(async move {
            _ = store_deliver
                .insert_certificate_delivered(&prev)
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(matches!(
            store.insert_pending_certificate(&cert.certificate).await,
            Ok(Some(_))
        ));
    }
}
