use rstest::rstest;
use std::future::IntoFuture;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{net::UdpSocket, time::Duration};
use test_log::test;
use tokio::sync::mpsc;
use tokio::{spawn, sync::oneshot};
use tokio_stream::StreamExt;
use tonic::transport::channel;
use tonic::transport::Uri;
use topos_core::api::shared::v1::checkpoints::TargetCheckpoint;
use topos_core::api::shared::v1::positions::TargetStreamPosition;
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
use topos_tce_storage::{Connection, RocksDBStorage, Storage};

const SOURCE_SUBNET_ID: topos_core::uci::SubnetId =
    topos_core::uci::SubnetId::from_array([1u8; 32]);
const TARGET_SUBNET_ID: topos_core::uci::SubnetId =
    topos_core::uci::SubnetId::from_array([2u8; 32]);
const PREV_CERTIFICATE_ID: CertificateId = CertificateId::from_array([4u8; 32]);

#[rstest]
#[timeout(Duration::from_secs(1))]
#[test(tokio::test)]
async fn runtime_can_dispatch_a_cert() {
    let (tx, rx) = oneshot::channel::<Certificate>();

    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let addr = socket.local_addr().ok().unwrap();

    // launch data store
    let mut temp_dir = std::path::PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
        .expect("Unable to read CARGO_TARGET_TMPDIR");
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    temp_dir.push(format!("./can_dispatch_test_{}", t.as_nanos()));

    let (storage, storage_client, _storage_stream) = {
        let storage = RocksDBStorage::with_isolation(&temp_dir).expect("valid rocksdb storage");
        Connection::build(Box::pin(async { Ok(storage) }))
    };
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
        Vec::new(),
    )
    .unwrap();

    // Send a dispatch command that will be push to the subnet A
    runtime_client.dispatch_certificate(cert.clone()).await;

    let certificate_received = rx.await.unwrap();

    assert_eq!(cert, certificate_received);
}

#[rstest]
#[timeout(Duration::from_secs(2))]
#[test(tokio::test)]
async fn can_catchup_with_old_certs() {
    let (tx, mut rx) = mpsc::channel::<Certificate>(16);

    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let addr = socket.local_addr().ok().unwrap();

    // launch data store
    let mut temp_dir = std::path::PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
        .expect("Unable to read CARGO_TARGET_TMPDIR");
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    temp_dir.push(format!("./can_dispatch_test_{}", t.as_nanos()));

    let certificates = create_certificate_chain(SOURCE_SUBNET_ID, TARGET_SUBNET_ID, 15);

    let (storage, storage_client, _storage_stream) = {
        let storage = RocksDBStorage::with_isolation(&temp_dir).expect("valid rocksdb storage");
        for certificate in &certificates {
            _ = storage.persist(certificate, None).await;
        }

        Connection::build(Box::pin(async { Ok(storage) }))
    };
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
                    target_subnet_ids: vec![TARGET_SUBNET_ID.into()],
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
        SOURCE_SUBNET_ID,
        Default::default(),
        Default::default(),
        &vec![TARGET_SUBNET_ID],
        0,
        Vec::new(),
    )
    .unwrap();

    // Send a dispatch command that will be push to the subnet A
    runtime_client.dispatch_certificate(cert.clone()).await;

    for (index, certificate) in certificates.iter().enumerate() {
        let certificate_received = rx
            .recv()
            .await
            .expect(&format!("Didn't received index {}", index));
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
    let mut temp_dir = std::path::PathBuf::from_str(env!("CARGO_TARGET_TMPDIR"))
        .expect("Unable to read CARGO_TARGET_TMPDIR");
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    temp_dir.push(format!("./can_dispatch_test_{}", t.as_nanos()));

    let certificates = create_certificate_chain(SOURCE_SUBNET_ID, TARGET_SUBNET_ID, 15);

    let (storage, storage_client, _storage_stream) = {
        let storage = RocksDBStorage::with_isolation(&temp_dir).expect("valid rocksdb storage");
        for certificate in &certificates {
            _ = storage.persist(certificate, None).await;
        }

        Connection::build(Box::pin(async { Ok(storage) }))
    };
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
                    target_subnet_ids: vec![TARGET_SUBNET_ID.into()],
                    positions: vec![
                        TargetStreamPosition {
                            certificate_id: None,
                            position: 5,
                            source_subnet_id: Some(SOURCE_SUBNET_ID.into()),
                            target_subnet_id: Some(TARGET_SUBNET_ID.into())
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
        SOURCE_SUBNET_ID,
        Default::default(),
        Default::default(),
        &vec![TARGET_SUBNET_ID],
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
            .expect(&format!("Didn't received index {}", index));
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

fn create_certificate_chain(
    source_subnet: topos_core::uci::SubnetId,
    target_subnet: topos_core::uci::SubnetId,
    number: usize,
) -> Vec<Certificate> {
    let mut certificates = Vec::new();
    let mut parent = None;

    for _ in 0..number {
        let cert = Certificate::new(
            parent.take().unwrap_or([0u8; 32]),
            source_subnet.clone(),
            Default::default(),
            Default::default(),
            &[target_subnet.clone()],
            0,
            Vec::new(),
        )
        .unwrap();
        parent = Some(cert.id.as_array().clone());
        certificates.push(cert);
    }

    certificates
}
