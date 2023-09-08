use std::{future::IntoFuture, time::Duration};

use futures::FutureExt;
use libp2p::{request_response::ResponseChannel, PeerId};
use mockall::mock;
use rstest::rstest;
use topos_core::api::grpc::tce::v1::{
    CheckpointMapFieldEntry, CheckpointRequest, CheckpointResponse, FetchCertificatesRequest,
    FetchCertificatesResponse,
};
use topos_p2p::{
    constant::SYNCHRONIZER_PROTOCOL, error::CommandExecutionError, NetworkClient, RetryPolicy,
    TransmissionResponse,
};
use topos_tce_gatekeeper::{GatekeeperClient, GatekeeperError};
use topos_tce_storage::{
    store::ReadStore, types::CertificateDelivered, CertificateSourceStreamPosition, Position,
};
use topos_test_sdk::{
    certificates::create_certificate_chain,
    storage::create_validator_store,
    tce::{create_network, NodeConfig},
};
use tracing::warn;
use uuid::Uuid;

use super::CheckpointSynchronizer;

mock! {
    NetworkClient {}

    impl NetworkClient for NetworkClient {
        fn send_request<T: std::fmt::Debug + Into<Vec<u8>> + 'static, R: TryFrom<Vec<u8>> + 'static>(
            &self,
            to: topos_p2p::PeerId,
            data: T,
            retry_policy: topos_p2p::RetryPolicy,
            protocol: &'static str,
        ) -> futures::future::BoxFuture<'static, Result<R, CommandExecutionError>>;

        fn respond_to_request<T: std::fmt::Debug + Into<Vec<u8>> + 'static>(
            &self,
            data: Result<T, ()>,
            channel: ResponseChannel<Result<TransmissionResponse, ()>>,
            protocol: &'static str,
        ) -> futures::future::BoxFuture<'static, Result<(), CommandExecutionError>>;
    }
}

mock! {
    GatekeeperClient {}

    #[async_trait::async_trait]
    impl GatekeeperClient for GatekeeperClient {
        async fn get_random_peers(&self, number: usize) -> Result<Vec<PeerId>, GatekeeperError>;
    }
}

use test_log::test;
#[test(tokio::test)]
async fn can_initiate_a_sync() {
    let peer_id = PeerId::random();

    let validator_store = create_validator_store::default().await;

    let subnet = topos_test_sdk::constants::SOURCE_SUBNET_ID_1;
    let certificates: Vec<CertificateDelivered> =
        create_certificate_chain(subnet, &[topos_test_sdk::constants::TARGET_SUBNET_ID_1], 1);
    let certificate = certificates.first().cloned().unwrap();
    let certificate_id = certificate.certificate.id;

    let mut client = MockNetworkClient::new();

    let certificates_checkpoint = certificates.clone();
    client
        .expect_send_request::<CheckpointRequest, CheckpointResponse>()
        .times(1)
        .returning(move |_, request, _, _| {
            warn!("Received checkpoint request from {}", peer_id);
            let checkpoint = CheckpointResponse {
                request_id: request.request_id,
                checkpoint_diff: certificates_checkpoint
                    .iter()
                    .map(|c| CheckpointMapFieldEntry {
                        key: c.certificate.source_subnet_id.to_string(),
                        value: vec![c.proof_of_delivery.clone().into()],
                    })
                    .collect(),
            };

            async move { Ok(checkpoint) }.boxed()
        });

    client
        .expect_send_request::<FetchCertificatesRequest, FetchCertificatesResponse>()
        .times(1)
        .returning(move |_, request, _, _| {
            warn!("Received Fetch certificates from {}", peer_id);
            let response = FetchCertificatesResponse {
                request_id: request.request_id,
                certificates: certificates
                    .iter()
                    .map(|c| c.certificate.clone().into())
                    .collect(),
            };
            async move { Ok(response) }.boxed()
        });

    let mut gatekeeper_client = MockGatekeeperClient::new();

    gatekeeper_client
        .expect_get_random_peers()
        .times(2)
        .returning(|_| Ok(vec![PeerId::random()]));

    let (_, mut sync, _) = CheckpointSynchronizer::builder()
        .set_network_client(Some(client))
        .set_gatekeeper_client(Some(gatekeeper_client))
        .set_store(Some(validator_store.clone()))
        .into_future()
        .await
        .unwrap();

    sync.initiate_request().await.unwrap();

    sync.network.checkpoint();
    sync.gatekeeper.checkpoint();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let value = validator_store
        .last_delivered_position_for_subnet(&subnet)
        .unwrap()
        .unwrap();

    assert_eq!(
        value,
        CertificateSourceStreamPosition {
            source_subnet_id: subnet,
            position: Position(0)
        }
    );

    assert_eq!(
        certificate,
        validator_store
            .get_certificate(&certificate_id)
            .unwrap()
            .unwrap()
    );

    assert!(validator_store
        .get_unverified_proof(&certificate_id)
        .unwrap()
        .is_none());
}

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
            value: vec![cert.proof_of_delivery.clone().into()],
        }],
    };

    let x: Vec<u8> = req.clone().into();
    let y: CheckpointResponse = x.try_into().unwrap();
    assert_eq!(y, req);
}

#[test(tokio::test)]
#[rstest]
#[timeout(Duration::from_secs(10))]
async fn network_test() {
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
    let (client, _, _) = cfg.bootstrap(&[boot_node.clone()]).await.unwrap();

    use topos_core::api::grpc::shared::v1::Uuid as APIUuid;

    let request_id: APIUuid = Uuid::new_v4().into();
    let req = FetchCertificatesRequest {
        request_id: Some(request_id),
        certificates: certificates
            .clone()
            .into_iter()
            .map(|c| c.certificate.id.try_into().unwrap())
            .collect(),
    };

    let res = client
        .send_request::<_, FetchCertificatesResponse>(
            boot_node.keypair.public().to_peer_id(),
            req,
            RetryPolicy::NoRetry,
            SYNCHRONIZER_PROTOCOL,
        )
        .await;

    assert!(res.is_ok());
    let res = res.unwrap();

    let expected = certificates
        .into_iter()
        .map(|c| c.certificate.try_into().unwrap())
        .collect::<Vec<topos_core::api::grpc::uci::v1::Certificate>>();

    assert_eq!(res.certificates, expected);
}
