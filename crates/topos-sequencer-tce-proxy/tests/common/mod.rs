use async_stream::stream;
use byteorder::ByteOrder;
use futures::{channel::oneshot, FutureExt, Stream, StreamExt};
use std::net::UdpSocket;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use topos_core::api::shared::v1::{CertificateId, SubnetId};
use topos_core::api::tce::v1::api_service_server::{ApiService, ApiServiceServer};
use topos_core::api::tce::v1::{
    watch_certificates_request, watch_certificates_response, GetSourceHeadRequest,
    GetSourceHeadResponse, SourceStreamPosition, SubmitCertificateRequest,
    SubmitCertificateResponse, WatchCertificatesRequest, WatchCertificatesResponse,
};
use topos_core::api::uci::v1::Certificate;
use tracing::debug;

const DEFAULT_CHANNEL_STREAM_CAPACITY: usize = 10;

struct TceMockServer {
    certificate_history: Arc<Mutex<Vec<Certificate>>>,
}

#[tonic::async_trait]
impl ApiService for TceMockServer {
    type WatchCertificatesStream =
        Pin<Box<dyn Stream<Item = Result<WatchCertificatesResponse, Status>> + Send + 'static>>;

    async fn submit_certificate(
        &self,
        request: Request<SubmitCertificateRequest>,
    ) -> Result<Response<SubmitCertificateResponse>, tonic::Status> {
        let request = request.into_inner();
        debug!(
            "TCE MOCK NODE: Certificate submitted to mock tce node: {:?}",
            &request
        );
        let mut history = self.certificate_history.lock().await;
        history.push(request.certificate.unwrap());
        Ok(Response::new(SubmitCertificateResponse {}))
    }

    async fn get_source_head(
        &self,
        request: Request<GetSourceHeadRequest>,
    ) -> Result<Response<GetSourceHeadResponse>, tonic::Status> {
        debug!("TCE MOCK NODE: Get source head certificate: {:?}", request);

        let request = request.into_inner();
        let mut history = self.certificate_history.lock().await;
        if history.is_empty() {
            // Add genesis certificate as dummy certificate
            // TODO Fix this with genesis certificate retrieved from topos subnet
            let dummy_certificate = Certificate {
                id: Default::default(),
                prev_id: Default::default(),
                source_subnet_id: request.subnet_id.clone(),
                target_subnets: vec![],
                ..Default::default()
            };
            history.push(dummy_certificate);
        }

        let last_certificate = history.last().unwrap();
        Ok(Response::new(GetSourceHeadResponse {
            position: Some(SourceStreamPosition {
                subnet_id: request.subnet_id.clone(),
                certificate_id: last_certificate.id.clone(),
                position: (history.len() - 1) as u64,
            }),
            certificate: Some(last_certificate.clone()),
        }))
    }

    async fn watch_certificates(
        &self,
        request: Request<tonic::Streaming<WatchCertificatesRequest>>,
    ) -> Result<Response<Self::WatchCertificatesStream>, tonic::Status> {
        let mut stream: Streaming<_> = request.into_inner();
        debug!("TCE node service watch certificates called");
        let (tx, mut rx) =
            mpsc::channel::<WatchCertificatesResponse>(DEFAULT_CHANNEL_STREAM_CAPACITY);

        let output = stream! {
            let mut counter: u32 = 0;
            debug!("TCE node service output stream functioning");
            loop {
                let source_subnet_id: SubnetId = [1u8; 32].into();
                debug!("TCE node service loop entered counter {}", counter);
                tokio::select! {
                    Some(watch_certificate_request) = stream.next() => {
                        debug!("TCE node service processing watch_certificate_request: {:?} counter {}", watch_certificate_request, counter);
                        let mut subnet_ids = vec![];
                        if let Some(watch_certificates_request::Command::OpenStream(open_stream)) = &watch_certificate_request.as_ref().unwrap().command {
                            subnet_ids = open_stream.subnet_ids.clone();
                        };
                        let tx = tx.clone();
                        // Task for generating TCE node service responses
                        tokio::spawn(async move {
                            debug!("TCE node service spawned task to send WatchCertificatesResponse");
                            loop {
                                debug!("TCE node service generating WatchCertificatesResponse, counter {}", counter);
                                if counter == 0 {
                                    debug!("TCE node service preparing WatchCertificatesResponse for StreamOpened call");
                                    // First time send Stream opened message
                                    let _ = tx.send(WatchCertificatesResponse {
                                        request_id: watch_certificate_request.as_ref().unwrap().request_id.clone(),
                                        event: Some(watch_certificates_response::Event::StreamOpened(watch_certificates_response::StreamOpened {subnet_ids: subnet_ids.clone()}))
                                    }).await;
                                } else {
                                    // Other times send certificate
                                    debug!("TCE node preparing WatchCertificatesResponse with CertificatePushed");
                                    let _ = tx.send(WatchCertificatesResponse {
                                        request_id: watch_certificate_request.as_ref().unwrap().request_id.clone(),
                                        event: Some(watch_certificates_response::Event::CertificatePushed(watch_certificates_response::CertificatePushed {certificate: Some(Certificate{
                                            source_subnet_id: Some(source_subnet_id.clone()),
                                            id: {

                                                let mut bytes = vec![0,0,0,0];
                                                byteorder::LittleEndian::write_u32(&mut bytes, counter);
                                                bytes.resize(32, 0);
                                                Some(CertificateId {
                                                value: bytes,
                                            })},
                                            prev_id: {
                                                let mut bytes = vec![0,0,0,0];
                                                byteorder::LittleEndian::write_u32(&mut bytes, counter-1);
                                                bytes.resize(32, 0);
                                                Some(CertificateId {
                                                value: bytes,
                                            })},
                                            target_subnets: vec![],
                                            ..Default::default()
                                        })}))}).await;
                                };
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                counter = counter+1;
                            }
                        });
                    }
                    Some(event) = rx.recv() => {
                        debug!("TCE node event received {:?}, yielding to tce outbound stream...", event);
                        yield Ok(event);
                    }
                }
                debug!("TCE node sleeping for 2 seconds");
            }
        };

        Ok(Response::new(
            Box::pin(output) as Self::WatchCertificatesStream
        ))
    }
}

pub async fn start_mock_tce_service() -> Result<
    (
        tokio::task::JoinHandle<()>,
        futures::channel::oneshot::Sender<()>,
        String,
    ),
    Box<dyn std::error::Error>,
> {
    debug!("Starting mock TCE node...");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let svc = ApiServiceServer::new(TceMockServer {
        certificate_history: Arc::new(Default::default()),
    });
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let addr = socket.local_addr().ok().unwrap();

    let service = tokio::spawn(async move {
        Server::builder()
            .add_service(svc)
            .serve_with_shutdown(addr.clone(), shutdown_rx.map(drop))
            .await
            .expect("mock node server successfully started")
    });
    debug!("Mock TCE node successfully started");

    Ok((service, shutdown_tx, addr.to_string()))
}
