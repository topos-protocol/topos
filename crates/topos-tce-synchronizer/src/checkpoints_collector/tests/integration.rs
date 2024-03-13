use std::time::Duration;

use rstest::rstest;
use test_log::test;
use topos_core::{
    api::grpc::tce::v1::{
        synchronizer_service_client::SynchronizerServiceClient,
        synchronizer_service_server::SynchronizerServiceServer, FetchCertificatesRequest,
    },
    types::CertificateDelivered,
};

use topos_test_sdk::{
    certificates::create_certificate_chain,
    tce::{create_network, NodeConfig},
};
use uuid::Uuid;

use crate::SynchronizerService;

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(5))]
async fn network_test() {
    let subnet = topos_test_sdk::constants::SOURCE_SUBNET_ID_1;
    let certificates: Vec<CertificateDelivered> =
        create_certificate_chain(subnet, &[topos_test_sdk::constants::TARGET_SUBNET_ID_1], 1);

    let boot_node = NodeConfig::from_seed(1);
    let cluster = create_network(5, &certificates[..]).await;
    let boot_node = cluster
        .get(&boot_node.keypair.public().to_peer_id())
        .unwrap()
        .node_config
        .clone();

    let cfg = NodeConfig {
        seed: 6,
        minimum_cluster_size: 1,
        ..Default::default()
    };

    let (client, _, _) = cfg
        .bootstrap(&[cfg.clone(), boot_node.clone()], None)
        .await
        .unwrap();

    use topos_core::api::grpc::shared::v1::Uuid as APIUuid;

    let peer = boot_node.keypair.public().to_peer_id();

    let mut client: SynchronizerServiceClient<_> = client
        .new_grpc_client::<SynchronizerServiceClient<_>, SynchronizerServiceServer<SynchronizerService>>(
            peer,
        )
        .await
        .unwrap();

    let request_id: APIUuid = Uuid::new_v4().into();
    let req = FetchCertificatesRequest {
        request_id: Some(request_id),
        certificates: certificates
            .clone()
            .into_iter()
            .map(|c| c.certificate.id.into())
            .collect(),
    };

    let res = client.fetch_certificates(req).await.unwrap().into_inner();

    let expected = certificates
        .into_iter()
        .map(|c| c.certificate.into())
        .collect::<Vec<topos_core::api::grpc::uci::v1::Certificate>>();

    assert_eq!(res.certificates, expected);
}
