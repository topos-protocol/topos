use async_stream::stream;
use futures::{channel::oneshot, FutureExt};
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::time::Duration;
use test_log::test;
use tokio::sync::mpsc;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use topos_api::shared;
use topos_api::shared::v1::checkpoints::TargetCheckpoint;
use topos_api::shared::v1::{CertificateId, SubnetId};
use topos_api::tce::v1::api_service_server::{ApiService, ApiServiceServer};
use topos_api::tce::v1::watch_certificates_request::{Command, OpenStream};
use topos_api::tce::v1::{
    GetSourceHeadRequest, GetSourceHeadResponse, SourceStreamPosition, SubmitCertificateRequest,
    SubmitCertificateResponse, WatchCertificatesRequest, WatchCertificatesResponse,
};
use topos_api::uci::v1::Certificate;
use uuid::Uuid;

#[test(tokio::test)]
async fn create_tce_layer() {
    struct TceServer;

    #[tonic::async_trait]
    impl ApiService for TceServer {
        type WatchCertificatesStream =
            Pin<Box<dyn Stream<Item = Result<WatchCertificatesResponse, Status>> + Send + 'static>>;

        async fn submit_certificate(
            &self,
            _request: Request<SubmitCertificateRequest>,
        ) -> Result<Response<SubmitCertificateResponse>, tonic::Status> {
            Ok(Response::new(SubmitCertificateResponse {}))
        }

        async fn get_source_head(
            &self,
            request: Request<GetSourceHeadRequest>,
        ) -> Result<Response<GetSourceHeadResponse>, tonic::Status> {
            let request = request.into_inner();
            let return_certificate_id = CertificateId {
                value: [02u8; 32].to_vec(),
            };
            let return_prev_certificate_id: CertificateId = CertificateId {
                value: [01u8; 32].to_vec(),
            };
            Ok(Response::new(GetSourceHeadResponse {
                position: Some(SourceStreamPosition {
                    subnet_id: request.subnet_id.clone(),
                    certificate_id: Some(return_certificate_id.clone()),
                    position: 0,
                }),
                certificate: Some(Certificate {
                    source_subnet_id: request.subnet_id,
                    id: Some(return_certificate_id.clone()),
                    prev_id: Some(return_prev_certificate_id.clone()),
                    target_subnets: Vec::new(),
                    ..Default::default()
                }),
            }))
        }

        async fn watch_certificates(
            &self,
            request: Request<tonic::Streaming<WatchCertificatesRequest>>,
        ) -> Result<Response<Self::WatchCertificatesStream>, tonic::Status> {
            let mut stream: Streaming<_> = request.into_inner();
            let (tx, mut rx) = mpsc::channel::<WatchCertificatesResponse>(10);

            let output = stream! {
                loop {
                    tokio::select! {
                        Some(_message) = stream.next() => {
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                let _ = tx.send(WatchCertificatesResponse {
                                    request_id: Some(Uuid::new_v4().into()),
                                    event: None

                                }).await;

                            });
                        }

                        Some(event) = rx.recv() => {
                            yield Ok(event);
                        }

                    }
                }
            };

            Ok(Response::new(
                Box::pin(output) as Self::WatchCertificatesStream
            ))
        }
    }

    let (tx, rx) = oneshot::channel();
    let svc = ApiServiceServer::new(TceServer);

    let jh = tokio::spawn(async move {
        Server::builder()
            .add_service(svc)
            .serve_with_shutdown("127.0.0.1:1340".parse().unwrap(), rx.map(drop))
            .await
            .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut client =
        topos_api::tce::v1::api_service_client::ApiServiceClient::connect("http://127.0.0.1:1340")
            .await
            .unwrap();

    let source_subnet_id: SubnetId = [1u8; 32].into();

    let prev_certificate_id: CertificateId = CertificateId {
        value: [01u8; 32].to_vec(),
    };
    let certificate_id: CertificateId = CertificateId {
        value: [02u8; 32].to_vec(),
    };

    let original_certificate = Certificate {
        source_subnet_id: Some(source_subnet_id.clone()),
        id: Some(certificate_id),
        prev_id: Some(prev_certificate_id),
        target_subnets: vec![],
        ..Default::default()
    };

    // Submit one certificate
    let response = client
        .submit_certificate(SubmitCertificateRequest {
            certificate: Some(original_certificate.clone()),
        })
        .await
        .map(|r| r.into_inner())
        .unwrap();
    assert_eq!(response, SubmitCertificateResponse {});

    // Test get source head certificate
    let response = client
        .get_source_head(GetSourceHeadRequest {
            subnet_id: Some(source_subnet_id.clone()),
        })
        .await
        .map(|r| r.into_inner())
        .unwrap();
    let expected_response = GetSourceHeadResponse {
        certificate: Some(original_certificate.clone()),
        position: Some(SourceStreamPosition {
            subnet_id: Some(source_subnet_id.clone()),
            certificate_id: original_certificate.id,
            position: 0,
        }),
    };
    assert_eq!(response, expected_response);

    let command = Some(Command::OpenStream(OpenStream {
        target_checkpoint: Some(TargetCheckpoint {
            target_subnet_ids: vec![source_subnet_id.clone()],
            positions: Vec::new(),
        }),
        source_checkpoint: None,
    }));
    let request_id: shared::v1::Uuid = Uuid::new_v4().into();
    let first_request = WatchCertificatesRequest {
        request_id: Some(request_id.clone()),
        command,
    };

    let mut first_request_short: WatchCertificatesRequest = OpenStream {
        target_checkpoint: Some(TargetCheckpoint {
            target_subnet_ids: vec![source_subnet_id],
            positions: Vec::new(),
        }),
        source_checkpoint: None,
    }
    .into();
    first_request_short.request_id = Some(request_id);

    assert_eq!(first_request, first_request_short);

    let outbound = stream! {
        yield first_request;
    };

    let mut stream = client
        .watch_certificates(outbound)
        .await
        .map(|r| r.into_inner())
        .unwrap();

    let message = stream.message().await.unwrap();
    assert!(matches!(message, Some(WatchCertificatesResponse { .. })));

    tx.send(()).unwrap();
    drop(stream);
    jh.await.unwrap();
}
