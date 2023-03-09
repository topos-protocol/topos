use futures::Stream;
use rstest::rstest;
use std::future::IntoFuture;
use std::{net::UdpSocket, time::Duration};
use test_log::test;
use tokio::sync::mpsc;
use tokio::{spawn, sync::oneshot};
use tokio_stream::StreamExt;
use tonic::transport::channel;
use tonic::transport::Uri;
use topos_core::api::shared::v1::checkpoints::TargetCheckpoint;
use topos_core::api::shared::v1::positions::TargetStreamPosition;
use topos_core::{
    api::tce::v1::{
        api_service_client::ApiServiceClient,
        watch_certificates_request::OpenStream,
        watch_certificates_response::{CertificatePushed, Event},
    },
    uci::Certificate,
};
use topos_tce_api::{Runtime, RuntimeEvent};
use topos_test_sdk::certificates::create_certificate_chain;
use topos_test_sdk::constants::*;
use topos_test_sdk::storage::storage_client;
use topos_test_sdk::tce::public_api::{create_public_api, PublicApiContext};

#[rstest]
#[timeout(Duration::from_secs(1))]
#[test(tokio::test)]
async fn runtime_can_dispatch_a_cert(
    #[future] create_public_api: (PublicApiContext, impl Stream<Item = RuntimeEvent>),
) {
    let (api_context, _) = create_public_api.await;
    let mut client = api_context.api_client;
    let (tx, rx) = oneshot::channel::<Certificate>();

    // This block represent a subnet A
    spawn(async move {
        let in_stream = async_stream::stream! {
            yield OpenStream {
                target_checkpoint: Some(TargetCheckpoint {
                    target_subnet_ids: vec![TARGET_SUBNET_ID_1.into()],
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
    tokio::time::sleep(Duration::from_millis(10)).await;

    let cert = topos_core::uci::Certificate::new(
        PREV_CERTIFICATE_ID,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1],
        0,
        Vec::new(),
    )
    .unwrap();

    // Send a dispatch command that will be push to the subnet A
    api_context.client.dispatch_certificate(cert.clone()).await;

    let certificate_received = rx.await.unwrap();
    assert_eq!(cert, certificate_received);
}

#[rstest]
#[timeout(Duration::from_secs(2))]
#[test(tokio::test)]
async fn can_catchup_with_old_certs(
    #[with(SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1, 15)]
    #[from(create_certificate_chain)]
    certificates: Vec<Certificate>,
) {
    let storage_client = storage_client::partial_1(certificates.clone());
    let (api_context, _) = create_public_api::partial_1(storage_client).await;

    let mut client = api_context.api_client;

    let (tx, mut rx) = mpsc::channel::<Certificate>(16);

    // This block represent a subnet A
    spawn(async move {
        let in_stream = async_stream::stream! {
            yield OpenStream {
                target_checkpoint: Some(TargetCheckpoint {
                    target_subnet_ids: vec![TARGET_SUBNET_ID_1.into()],
                    positions: Vec::new()
                }),
                source_checkpoint: None
            }.into()
        };

        let response = client.watch_certificates(in_stream).await.unwrap();

        let mut resp_stream = response.into_inner();

        while let Some(received) = resp_stream.next().await {
            let received = received.unwrap();
            if let Some(Event::CertificatePushed(CertificatePushed {
                certificate: Some(certificate),
            })) = received.event
            {
                _ = tx.send(certificate.try_into().unwrap()).await;
            }
        }
    });

    // Wait for client to be ready
    tokio::time::sleep(Duration::from_millis(100)).await;

    let last = certificates.last().map(|c| c.id).unwrap();
    let cert = topos_core::uci::Certificate::new(
        last,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1],
        0,
        Vec::new(),
    )
    .unwrap();

    // Send a dispatch command that will be push to the subnet A
    api_context.client.dispatch_certificate(cert.clone()).await;

    for (index, certificate) in certificates.iter().enumerate() {
        let certificate_received = rx
            .recv()
            .await
            .unwrap_or_else(|| panic!("Didn't received index {}", index));
        assert_eq!(
            certificate, &certificate_received,
            "Certificate at index {} not received",
            index
        );
    }

    let certificate_received = rx.recv().await.unwrap();
    assert_eq!(cert, certificate_received);
}

#[rstest]
#[timeout(Duration::from_secs(2))]
#[test(tokio::test)]
async fn can_catchup_with_old_certs_with_position() {
    let (tx, mut rx) = mpsc::channel::<Certificate>(16);

    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let addr = socket.local_addr().ok().unwrap();

    // launch data store
    let certificates = create_certificate_chain(SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1, 15);

    let (_, (storage, storage_client, _storage_stream)) = topos_test_sdk::storage::create_rocksdb(
        "can_catchup_with_old_certs_with_position",
        certificates.clone(),
    )
    .await;

    let _storage_join_handle = spawn(storage.into_future());

    let (runtime_client, _launcher) = Runtime::builder()
        .storage(storage_client)
        .serve_addr(addr)
        .build_and_launch()
        .await;

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
                    target_subnet_ids: vec![TARGET_SUBNET_ID_1.into()],
                    positions: vec![
                        TargetStreamPosition {
                            certificate_id: None,
                            position: 5,
                            source_subnet_id: Some(SOURCE_SUBNET_ID_1.into()),
                            target_subnet_id: Some(TARGET_SUBNET_ID_1.into())
                        }
                    ]
                }),
                source_checkpoint: None
            }.into()
        };

        let response = client.watch_certificates(in_stream).await.unwrap();

        let mut resp_stream = response.into_inner();

        while let Some(received) = resp_stream.next().await {
            let received = received.unwrap();
            if let Some(Event::CertificatePushed(CertificatePushed {
                certificate: Some(certificate),
            })) = received.event
            {
                _ = tx.send(certificate.try_into().unwrap()).await;
            }
        }
    });

    // Wait for client to be ready
    tokio::time::sleep(Duration::from_millis(100)).await;

    let last = certificates.last().map(|c| c.id).unwrap();
    let cert = topos_core::uci::Certificate::new(
        last,
        SOURCE_SUBNET_ID_1,
        Default::default(),
        Default::default(),
        &[TARGET_SUBNET_ID_1],
        0,
        Vec::new(),
    )
    .unwrap();

    // Send a dispatch command that will be push to the subnet A
    runtime_client.dispatch_certificate(cert.clone()).await;

    for (index, certificate) in certificates.iter().skip(5).enumerate() {
        let certificate_received = rx
            .recv()
            .await
            .unwrap_or_else(|| panic!("Didn't received index {}", index));
        assert_eq!(
            certificate, &certificate_received,
            "Certificate at index {} not received",
            index
        );
    }

    let certificate_received = rx.recv().await.unwrap();
    assert_eq!(cert, certificate_received);
}
#[test(tokio::test)]
#[ignore = "not yet implemented"]
async fn can_listen_for_multiple_subnet_id() {}
