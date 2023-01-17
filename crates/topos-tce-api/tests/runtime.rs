use std::{net::UdpSocket, time::Duration};
use test_log::test;
use tokio::{spawn, sync::oneshot};
use tokio_stream::StreamExt;
use tonic::transport::channel;
use tonic::transport::Uri;
use topos_core::api::shared::v1::SubnetId;
use topos_core::uci::CertificateId;
use topos_core::{
    api::tce::v1::{
        api_service_client::ApiServiceClient,
        watch_certificates_request::OpenStream,
        watch_certificates_response::{CertificatePushed, Event},
    },
    uci::{Address, Amount, Certificate, CrossChainTransaction, CrossChainTransactionData},
};
use topos_tce_api::Runtime;

const SOURCE_SUBNET_ID: topos_core::uci::SubnetId = [1u8; 32];
const TARGET_SUBNET_ID: topos_core::uci::SubnetId = [2u8; 32];
const PREV_CERTIFICATE_ID: CertificateId = CertificateId::from_array([4u8; 32]);
const SENDER_ID: Address = [6u8; 20];
const RECEIVER_ID: Address = [7u8; 20];

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
            yield OpenStream { subnet_ids: vec![SubnetId {value: TARGET_SUBNET_ID.into()}] }.into();
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
                    _ = tx.send(certificate.into());
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
        vec![CrossChainTransaction {
            target_subnet_id: TARGET_SUBNET_ID,
            transaction_data: CrossChainTransactionData::AssetTransfer {
                sender: SENDER_ID,
                receiver: RECEIVER_ID,
                symbol: "TST_SUBNET_".to_string(),
                amount: Amount::from(1000),
            },
        }],
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
