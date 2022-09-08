use std::time::Duration;
use test_log::test;
use tokio::{spawn, sync::oneshot};
use tokio_stream::StreamExt;
use tonic::transport::channel;
use topos_core::{
    api::tce::v1::{
        api_service_client::ApiServiceClient,
        watch_certificates_request::OpenStream,
        watch_certificates_response::{CertificatePushed, Event},
    },
    uci::Certificate,
};
use topos_tce_api::Runtime;

#[test(tokio::test)]
async fn runtime_can_dispatch_a_cert() {
    let (tx, rx) = oneshot::channel::<Certificate>();

    let (runtime_client, launcher) = Runtime::builder().build();

    spawn(launcher.launch());

    // Wait for server to boot
    tokio::time::sleep(Duration::from_millis(100)).await;

    // This block represent a subnet A
    spawn(async move {
        let channel = channel::Endpoint::from_static("http://127.0.0.1:1340").connect_lazy();
        let mut client = ApiServiceClient::new(channel);
        let in_stream = async_stream::stream! {
            yield OpenStream { subnet_ids: vec!["subnet_id".into()] }.into();
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
        "subnet_id".to_string(),
        Vec::new(),
    );

    // Send a dispatch command that will be push to the subnet A
    runtime_client.dispatch_certificate(cert.clone()).await;

    let certificate_received = rx.await.unwrap();

    assert_eq!(cert, certificate_received);
}

#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn can_listen_for_multiple_subnet_id() {}
