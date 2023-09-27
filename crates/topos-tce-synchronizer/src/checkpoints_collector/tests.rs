use topos_core::{
    api::grpc::tce::v1::{CheckpointMapFieldEntry, CheckpointRequest, CheckpointResponse},
    types::CertificateDelivered,
};
use topos_test_sdk::certificates::create_certificate_chain;

use uuid::Uuid;

use super::CheckpointSynchronizer;

mod integration;

#[test]
fn encode() {
    use topos_core::api::grpc::shared::v1::Uuid as APIUuid;
    let request_id: APIUuid = Uuid::new_v4().into();
    let req = CheckpointRequest {
        request_id: Some(request_id),
        checkpoint: vec![],
    };

    let x: Vec<u8> = req.clone().into();
    let y: CheckpointRequest = x.try_into().unwrap();
    assert_eq!(y, req);

    let subnet = topos_test_sdk::constants::SOURCE_SUBNET_ID_1;
    let certificates: Vec<CertificateDelivered> =
        create_certificate_chain(subnet, &[topos_test_sdk::constants::TARGET_SUBNET_ID_1], 1);

    let cert = certificates.first().cloned().unwrap();
    let request_id: APIUuid = Uuid::new_v4().into();
    let req = CheckpointResponse {
        request_id: Some(request_id),
        checkpoint_diff: vec![CheckpointMapFieldEntry {
            key: subnet.to_string(),
            value: vec![cert.proof_of_delivery.into()],
        }],
    };

    let x: Vec<u8> = req.clone().into();
    let y: CheckpointResponse = x.try_into().unwrap();
    assert_eq!(y, req);
}

#[test]
fn sync_same_certificate_twice() {
    //
}

#[test]
fn sync_unordered_certificates() {}

#[test]
fn sync_conflicting_certificate() {}

#[test]
fn fetch_certificate_failure() {}

#[test]
fn missing_certificate_for_pod() {}
