use std::{net::UdpSocket, time::Duration};
use test_log::test;
use tokio::{spawn, sync::oneshot};
use tokio_stream::StreamExt;
use tonic::transport::channel;
use tonic::transport::Uri;
use topos_core::{
    api::tce::v1::{
        api_service_client::ApiServiceClient,
        watch_certificates_request::OpenStream,
        watch_certificates_response::{CertificatePushed, Event},
    },
    uci::{Address, Amount, Certificate, CrossChainTransaction, CrossChainTransactionData},
};
use topos_tce_api::Runtime;

#[test(tokio::test)]
async fn runtime_can_dispatch_a_cert() {
    let target_subnet_id = "subneta".to_string();
    let source_subnet_id = "subnetb".to_string();
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
    let subnet_a = target_subnet_id.clone();
    spawn(async move {
        let channel = channel::Channel::builder(uri).connect_lazy();
        let mut client = ApiServiceClient::new(channel);
        let in_stream = async_stream::stream! {
            yield OpenStream { subnet_ids: vec![subnet_a.into()] }.into();
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
        "previous_cert".to_string(),
        source_subnet_id.to_string(),
        vec![CrossChainTransaction {
            target_subnet_id: target_subnet_id.clone().into(),
            transaction_data: CrossChainTransactionData::AssetTransfer {
                asset_id: "TST_SUBNET_".to_string() + &target_subnet_id,
                amount: Amount::from(1000),
            },
            recipient_addr: Address::from("0x0000000000000000000000000000000000000002"),
            sender_addr: Address::from("0x0000000000000000000000000000000000000001"),
        }],
    );

    // Send a dispatch command that will be push to the subnet A
    runtime_client.dispatch_certificate(cert.clone()).await;

    let certificate_received = rx.await.unwrap();

    assert_eq!(cert, certificate_received);
}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn can_listen_for_multiple_subnet_id() {}
