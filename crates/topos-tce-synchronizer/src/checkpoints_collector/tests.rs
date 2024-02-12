use std::time::Duration;

use rstest::rstest;
use topos_core::{
    api::grpc::tce::v1::{
        synchronizer_service_client::SynchronizerServiceClient,
        synchronizer_service_server::SynchronizerServiceServer, CheckpointMapFieldEntry,
        CheckpointRequest, CheckpointResponse, FetchCertificatesRequest,
    },
    types::CertificateDelivered,
};

use topos_p2p::GrpcRouter;
use topos_test_sdk::{
    certificates::create_certificate_chain,
    storage::{create_fullnode_store, create_validator_store},
    tce::{create_network, NodeConfig},
};

use uuid::Uuid;

use crate::SynchronizerService;

mod integration;

#[test]
fn encode() {
    use topos_core::api::grpc::shared::v1::Uuid as APIUuid;
    let request_id: APIUuid = Uuid::new_v4().into();
    let req = CheckpointRequest {
        request_id: Some(request_id),
        checkpoint: vec![],
        limit_per_subnet: 100,
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

#[rstest]
#[test_log::test(tokio::test)]
#[timeout(Duration::from_secs(10))]
async fn check_fetch_certificates() {
    let subnet = topos_test_sdk::constants::SOURCE_SUBNET_ID_1;
    let certificates: Vec<CertificateDelivered> =
        create_certificate_chain(subnet, &[topos_test_sdk::constants::TARGET_SUBNET_ID_1], 1);

    let boot_node = NodeConfig::from_seed(1);
    let cluster = create_network(5, certificates.clone()).await;
    let boot_node = cluster
        .get(&boot_node.keypair.public().to_peer_id())
        .unwrap()
        .node_config
        .clone();

    let cfg = NodeConfig {
        seed: 6,
        minimum_cluster_size: 3,
        ..Default::default()
    };

    let fullnode_store = create_fullnode_store(vec![]).await;
    let validator_store =
        create_validator_store(vec![], futures::future::ready(fullnode_store.clone())).await;

    let router = GrpcRouter::new(tonic::transport::Server::builder()).add_service(
        SynchronizerServiceServer::new(SynchronizerService {
            validator_store: validator_store.clone(),
        }),
    );

    let (client, _, _) = cfg
        .bootstrap(&[boot_node.clone()], Some(router))
        .await
        .unwrap();

    use topos_core::api::grpc::shared::v1::Uuid as APIUuid;

    let request_id: APIUuid = Uuid::new_v4().into();
    let req = FetchCertificatesRequest {
        request_id: Some(request_id),
        certificates: certificates
            .clone()
            .into_iter()
            .map(|c| c.certificate.id.into())
            .collect(),
    };

    let mut client: SynchronizerServiceClient<_> = client
        .new_grpc_client::<SynchronizerServiceClient<_>, SynchronizerServiceServer<SynchronizerService>>(boot_node.keypair.public().to_peer_id())
        .await
        .unwrap();

    let res = client.fetch_certificates(req).await;
    assert!(res.is_ok());
    let res = res.unwrap().into_inner();

    let expected = certificates
        .into_iter()
        .map(|c| c.certificate.into())
        .collect::<Vec<topos_core::api::grpc::uci::v1::Certificate>>();

    assert_eq!(res.certificates, expected);
}

#[test]
fn sync_unordered_certificates() {}

#[test]
fn sync_conflicting_certificate() {}

#[test]
fn fetch_certificate_failure() {}

#[test]
fn missing_certificate_for_pod() {}
