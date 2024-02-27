use base64ct::{Base64, Encoding};
use rstest::rstest;
use std::sync::Arc;
use test_log::test;
use topos_core::api::grpc::tce::v1::{GetLastPendingCertificatesRequest, LastPendingCertificate};
use topos_core::uci::Certificate;
use topos_test_sdk::{
    certificates::create_certificate_chain,
    constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1},
    storage::{create_fullnode_store, create_validator_store, storage_client},
    tce::public_api::{broadcast_stream, create_public_api},
};

use topos_tce_storage::validator::ValidatorStore;

#[rstest]
#[test(tokio::test)]
async fn fetch_latest_pending_certificates() {
    let fullnode_store = create_fullnode_store(&[]).await;
    let validator_store: Arc<ValidatorStore> =
        create_validator_store(&[], futures::future::ready(fullnode_store.clone())).await;

    let (api_context, _) = create_public_api(
        storage_client(&[]),
        broadcast_stream(),
        futures::future::ready(validator_store.clone()),
    )
    .await;
    let mut client = api_context.api_client;
    let certificates = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 2);

    let expected = certificates[1].certificate.clone();

    assert!(validator_store
        .insert_pending_certificate(&certificates[1].certificate)
        .await
        .unwrap()
        .is_none());

    assert!(validator_store
        .insert_pending_certificate(&certificates[0].certificate)
        .await
        .unwrap()
        .is_some());

    let mut res = client
        .get_last_pending_certificates(GetLastPendingCertificatesRequest {
            subnet_ids: vec![SOURCE_SUBNET_ID_1.into()],
        })
        .await
        .unwrap()
        .into_inner();

    let res: LastPendingCertificate = res
        .last_pending_certificate
        .remove(&Base64::encode_string(SOURCE_SUBNET_ID_1.as_array()))
        .unwrap();

    let res: Certificate = res.value.unwrap().try_into().unwrap();

    assert_eq!(res, expected);
}

#[rstest]
#[test(tokio::test)]
async fn fetch_latest_pending_certificates_with_conflicts() {
    let fullnode_store = create_fullnode_store(&[]).await;
    let validator_store: Arc<ValidatorStore> =
        create_validator_store(&[], futures::future::ready(fullnode_store.clone())).await;

    let (api_context, _) = create_public_api(
        storage_client(&[]),
        broadcast_stream(),
        futures::future::ready(validator_store.clone()),
    )
    .await;
    let mut client = api_context.api_client;
    let mut certificates = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 3);

    certificates[2].certificate.prev_id = certificates[1].certificate.prev_id;

    let expected = certificates[2].certificate.clone();

    for certificate in certificates.iter().skip(1) {
        assert!(validator_store
            .insert_pending_certificate(&certificate.certificate)
            .await
            .unwrap()
            .is_none());
    }

    assert!(validator_store
        .insert_pending_certificate(&certificates[0].certificate)
        .await
        .unwrap()
        .is_some());

    let mut res = client
        .get_last_pending_certificates(GetLastPendingCertificatesRequest {
            subnet_ids: vec![SOURCE_SUBNET_ID_1.into()],
        })
        .await
        .unwrap()
        .into_inner();

    let res: LastPendingCertificate = res
        .last_pending_certificate
        .remove(&Base64::encode_string(SOURCE_SUBNET_ID_1.as_array()))
        .unwrap();

    let res: Certificate = res.value.unwrap().try_into().unwrap();

    assert_eq!(res, expected);
}
