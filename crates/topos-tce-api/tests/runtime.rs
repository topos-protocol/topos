use rstest::rstest;
use std::{net::UdpSocket, time::Duration};
use test_log::test;
use tokio::{spawn, sync::oneshot};
use tokio_stream::StreamExt;
use tonic::transport::channel;
use tonic::transport::Uri;
use topos_core::api::shared::v1::checkpoints::TargetCheckpoint;
use topos_core::uci::CertificateId;
use topos_core::{
    api::tce::v1::{
        api_service_client::ApiServiceClient,
        watch_certificates_request::OpenStream,
        watch_certificates_response::{CertificatePushed, Event},
    },
    uci::Certificate,
};
use topos_tce_api::Runtime;

const SOURCE_SUBNET_ID: topos_core::uci::SubnetId = [1u8; 32];
const TARGET_SUBNET_ID: topos_core::uci::SubnetId = [2u8; 32];
const PREV_CERTIFICATE_ID: CertificateId = CertificateId::from_array([4u8; 32]);

#[rstest]
#[timeout(Duration::from_secs(1))]
#[test(tokio::test)]
async fn runtime_can_dispatch_a_cert() {
    let (tx, rx) = oneshot::channel::<Certificate>();

    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let addr = socket.local_addr().ok().unwrap();

    let (runtime_client, _launcher) = Runtime::builder().serve_addr(addr).build_and_launch().await;

    // Wait for server to boot
    tokio::time::sleep(Duration::from_millis(100)).await;

    let uri = Uri::builder()
        .path_and_query("/")
        .authority(addr.to_string())
        .scheme("http")
        .build()
        .unwrap();

    // This block represent a subnet A
    spawn(async move {
        let channel = channel::Channel::builder(uri).connect_lazy();
        let mut client = ApiServiceClient::new(channel);
        let in_stream = async_stream::stream! {
            yield OpenStream {
                target_checkpoint: Some(TargetCheckpoint {
                    target_subnet_ids: vec![TARGET_SUBNET_ID.into()],
                    positions: Vec::new()
                }),
                source_checkpoint: None
            }.into()
        };

        let response = client.watch_certificates(in_stream).await.unwrap();

        let mut resp_stream = response.into_inner();

        let mut tx = Some(tx);
        while let Some(received) = resp_stream.next().await {
            let received = received.unwrap();
            if let Some(Event::CertificatePushed(CertificatePushed {
                certificate: Some(certificate),
            })) = received.event
            {
                if let Some(tx) = tx.take() {
                    _ = tx.send(certificate.try_into().unwrap());
                } else {
                    panic!("Double certificate sent");
                }
            }
        }
    });

    // Wait for client to be ready
    tokio::time::sleep(Duration::from_millis(100)).await;

    let cert = topos_core::uci::Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID,
        Default::default(),
        Default::default(),
        &vec![TARGET_SUBNET_ID],
        0,
    )
    .unwrap();

    // Send a dispatch command that will be push to the subnet A
    runtime_client.dispatch_certificate(cert.clone()).await;

    let certificate_received = rx.await.unwrap();

    assert_eq!(cert, certificate_received);
}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn can_listen_for_multiple_subnet_id() {}
