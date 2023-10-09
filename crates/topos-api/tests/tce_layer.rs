use async_stream::stream;
use bytes::Bytes;
use futures::{channel::oneshot, FutureExt};
use futures::{Stream, StreamExt};
use http::uri::{Parts, PathAndQuery};
use http::Uri;
use prost::bytes::{Buf, BufMut, BytesMut};
use prost::Message;
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::pin::Pin;
use std::str::FromStr;
use std::time::Duration;
use test_log::test;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio_util::io::ReaderStream;
use tonic::body::BoxBody;
use tonic::client::GrpcService;
use tonic::codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder};
use tonic::transport::{Channel, Endpoint};
use tonic::IntoRequest;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use topos_api::grpc::shared::v1::checkpoints::TargetCheckpoint;
use topos_api::grpc::shared::v1::positions::SourceStreamPosition;
use topos_api::grpc::shared::v1::{CertificateId, SubnetId};
use topos_api::grpc::tce::v1::api_service_server::{ApiService, ApiServiceServer};
use topos_api::grpc::tce::v1::synchronizer_service_client::SynchronizerServiceClient;
use topos_api::grpc::tce::v1::synchronizer_service_server::{
    SynchronizerService, SynchronizerServiceServer,
};
use topos_api::grpc::tce::v1::watch_certificates_request::{Command, OpenStream};
use topos_api::grpc::tce::v1::{
    CheckpointRequest, CheckpointResponse, FetchCertificatesRequest, FetchCertificatesResponse,
    GetLastPendingCertificatesRequest, GetLastPendingCertificatesResponse, GetSourceHeadRequest,
    GetSourceHeadResponse, LastPendingCertificate, SubmitCertificateRequest,
    SubmitCertificateResponse, WatchCertificatesRequest, WatchCertificatesResponse,
};
use topos_api::grpc::uci::v1::Certificate;
use topos_api::grpc::{shared, GrpcClient};
use tower::service_fn;
use uuid::Uuid;

use topos_test_sdk::constants::*;

#[test(tokio::test)]
async fn create_tce_layer() {
    struct TceServer;
    use base64ct::{Base64, Encoding};

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
            let return_certificate_id: CertificateId = CERTIFICATE_ID_2.into();
            let return_prev_certificate_id: CertificateId = CERTIFICATE_ID_1.into();
            Ok(Response::new(GetSourceHeadResponse {
                position: Some(SourceStreamPosition {
                    source_subnet_id: request.subnet_id.clone(),
                    certificate_id: Some(return_certificate_id.clone()),
                    position: 0,
                }),
                certificate: Some(Certificate {
                    source_subnet_id: request.subnet_id,
                    id: Some(return_certificate_id),
                    prev_id: Some(return_prev_certificate_id),
                    target_subnets: Vec::new(),
                    ..Default::default()
                }),
            }))
        }

        async fn get_last_pending_certificates(
            &self,
            request: Request<GetLastPendingCertificatesRequest>,
        ) -> Result<Response<GetLastPendingCertificatesResponse>, Status> {
            let request = request.into_inner();
            let subnet_ids = request.subnet_ids;

            let return_certificate_id: CertificateId = CERTIFICATE_ID_2.into();
            let return_prev_certificate_id: CertificateId = CERTIFICATE_ID_1.into();

            let mut map = HashMap::new();
            for subnet_id in subnet_ids {
                map.insert(
                    Base64::encode_string(&subnet_id.value),
                    LastPendingCertificate {
                        value: Some(Certificate {
                            source_subnet_id: subnet_id.into(),
                            id: Some(return_certificate_id.clone()),
                            prev_id: Some(return_prev_certificate_id.clone()),
                            target_subnets: Vec::new(),
                            ..Default::default()
                        }),
                        index: 0,
                    },
                );
            }
            Ok(Response::new(GetLastPendingCertificatesResponse {
                last_pending_certificate: map,
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

    let mut client = topos_api::grpc::tce::v1::api_service_client::ApiServiceClient::connect(
        "http://127.0.0.1:1340",
    )
    .await
    .unwrap();

    let source_subnet_id: SubnetId = SOURCE_SUBNET_ID_1.into();

    let prev_certificate_id: CertificateId = CERTIFICATE_ID_1.into();
    let certificate_id: CertificateId = CERTIFICATE_ID_2.into();

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
            source_subnet_id: Some(source_subnet_id.clone()),
            certificate_id: original_certificate.id.clone(),
            position: 0,
        }),
    };
    assert_eq!(response, expected_response);

    // Test last pending certificate
    let response = client
        .get_last_pending_certificates(GetLastPendingCertificatesRequest {
            subnet_ids: vec![source_subnet_id.clone()],
        })
        .await
        .map(|r| r.into_inner())
        .unwrap();

    let mut expected_last_pending_certificate_ids = HashMap::new();
    expected_last_pending_certificate_ids.insert(
        Base64::encode_string(&source_subnet_id.value),
        LastPendingCertificate {
            value: Some(original_certificate.clone()),
            index: 0,
        },
    );

    let expected_response = GetLastPendingCertificatesResponse {
        last_pending_certificate: expected_last_pending_certificate_ids,
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
        request_id: Some(request_id),
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

#[rstest]
#[test(tokio::test)]
async fn create_grpc_client() {
    let entrypoint = Endpoint::from_static("http://127.0.0.1:1340").connect_lazy();

    let _client = SynchronizerServiceClient::init(entrypoint);
}

enum TestMsg {
    Fetch(FetchCertificatesRequest),
    Checkpoint(CheckpointRequest),
}

#[rstest::rstest]
#[tokio::test]
#[timeout(Duration::from_secs(1))]
async fn encode_into_enum() {
    let request_id: shared::v1::Uuid = Uuid::new_v4().into();
    let msg = CheckpointRequest {
        request_id: Some(request_id),
        checkpoint: Vec::new(),
    };
    #[derive(Debug)]
    pub struct TransparentEncoder(PhantomData<Vec<u8>>);

    impl Encoder for TransparentEncoder {
        type Error = Status;
        type Item = Vec<u8>;

        fn encode(&mut self, item: Self::Item, buf: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
            buf.writer()
                .write_all(&item)
                .map_err(|e| Status::internal(e.to_string()))?;
            Ok(())
        }
    }

    #[derive(Debug)]
    pub struct TransparentDecoder(PhantomData<Vec<u8>>);

    impl Decoder for TransparentDecoder {
        type Error = Status;
        type Item = Vec<u8>;

        fn decode(&mut self, buf: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
            let mut result: Vec<u8> = vec![];
            buf.reader()
                .read_to_end(&mut result)
                .map_err(|e| Status::internal(e.to_string()))?;

            Ok(Some(result))
        }
    }

    #[derive(Debug, Clone)]
    pub struct TransparentCodec(PhantomData<(Vec<u8>, Vec<u8>)>);

    impl Default for TransparentCodec {
        fn default() -> Self {
            Self(PhantomData)
        }
    }

    impl Codec for TransparentCodec {
        type Decode = Vec<u8>;
        type Decoder = TransparentDecoder;
        type Encode = Vec<u8>;
        type Encoder = TransparentEncoder;

        fn encoder(&mut self) -> Self::Encoder {
            TransparentEncoder(PhantomData)
        }

        fn decoder(&mut self) -> Self::Decoder {
            TransparentDecoder(PhantomData)
        }
    }

    let codec = TransparentCodec::default();

    #[derive(Serialize, Deserialize)]
    struct Msg {
        uri: String,
        msg: Bytes,
    }
    const BUFFER_SIZE: usize = 8 * 1024;
    // let msg = msg.encode_to_vec();
    const HEADER_SIZE: usize =
        // compression flag
        std::mem::size_of::<u8>() +
    // data length
    std::mem::size_of::<u32>();

    let (client, server) = tokio::io::duplex(BUFFER_SIZE);
    // let (sender, mut recv) = mpsc::channel(100);
    let s = SyncService {};
    let mut service = SynchronizerServiceServer::new(s);

    #[derive(Serialize, Deserialize)]
    struct NetworkMsg {
        uri: Bytes,
        body: Bytes,
    }

    let (client, server) = tokio::io::duplex(BUFFER_SIZE);
    tokio::spawn(async move {
        let mut serv = h2::server::handshake(server).await.unwrap();
        loop {
            if let Some(request) = serv.accept().await {
                println!("REQ: {:?}", request);
            }
        }
    });

    let mut client = Some(client);
    let dest = Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector_lazy(service_fn(move |_: Uri| {
            let client = client.take();

            async move {
                if let Some(client) = client {
                    Ok(client)
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Client already taken",
                    ))
                }
            }
        }));

    let mut client_1 = SynchronizerServiceClient::new(dest.clone());
    client_1
        .fetch_checkpoint(CheckpointRequest {
            request_id: Some(request_id),
            checkpoint: vec![],
        })
        .await;

    let mut client_2 = SynchronizerServiceClient::new(dest);
    client_2
        .fetch_checkpoint(CheckpointRequest {
            request_id: Some(request_id),
            checkpoint: vec![],
        })
        .await;
    //             let (mut request, mut respond) = request.unwrap();
    //             let uri_bytes = Bytes::from(request.uri().to_string());
    //             // let uri = request.uri().clone();
    //             let body = request.body_mut();
    //
    //             let mut the_data = BytesMut::new();
    //             while let Some(data) = body.data().await {
    //                 let data = data.unwrap();
    //                 the_data.put(&data[..]);
    //
    //                 let _ = body.flow_control().release_capacity(data.len());
    //             }
    //
    //             let req = NetworkMsg {
    //                 uri: uri_bytes,
    //                 body: the_data.freeze(),
    //             };
    //
    //             let data = bincode::serialize(&req).unwrap();
    //             drop(body);
    //
    //             let recover_data: NetworkMsg = bincode::deserialize(&data).unwrap();
    //
    //             let msg = http_body::Full::new(recover_data.body);
    //             let mut req = http::Request::new(msg);
    //
    //             *req.uri_mut() = Uri::try_from(&recover_data.uri[..]).unwrap();
    //
    //             // let (parts, body) = req.into_parts();
    //             // use http_body_util::BodyExt;
    //
    //             sender.send(req).await;
    //
    //             println!("Received request: {:?}", request);
    //         }
    //     }
    // });

    // let mut client = Some(client);
    // let dest = Endpoint::try_from("http://[::]:50051")
    //     .unwrap()
    //     .connect_with_connector(service_fn(move |_: Uri| {
    //         let client = client.take();
    //
    //         async move {
    //             if let Some(client) = client {
    //                 Ok(client)
    //             } else {
    //                 Err(std::io::Error::new(
    //                     std::io::ErrorKind::Other,
    //                     "Client already taken",
    //                 ))
    //             }
    //         }
    //     }))
    //     .await
    //     .unwrap();
    //
    // let mut client = SynchronizerServiceClient::new(dest);
    // client
    //     .fetch_checkpoint(CheckpointRequest {
    //         request_id: Some(request_id),
    //         checkpoint: vec![],
    //     })
    //     .await;
    struct SyncService {}

    #[tonic::async_trait]
    impl SynchronizerService for SyncService {
        async fn fetch_checkpoint(
            &self,
            req: tonic::Request<CheckpointRequest>,
        ) -> Result<tonic::Response<CheckpointResponse>, Status> {
            println!("OK in server");
            Err(Status::unimplemented(""))
        }

        async fn fetch_certificates(
            &self,
            req: tonic::Request<FetchCertificatesRequest>,
        ) -> Result<tonic::Response<FetchCertificatesResponse>, Status> {
            Err(Status::unimplemented(""))
        }
    }
}
