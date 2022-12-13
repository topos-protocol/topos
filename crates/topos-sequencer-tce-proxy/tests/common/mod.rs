use async_stream::stream;
use futures::{channel::oneshot, FutureExt, Stream, StreamExt};
use std::net::UdpSocket;
use std::pin::Pin;
use tokio::sync::mpsc;
use tonic::{transport::Server, Request, Response, Status, Streaming};
use topos_core::api::tce::v1::api_service_server::{ApiService, ApiServiceServer};
use topos_core::api::tce::v1::{
    watch_certificates_request, watch_certificates_response, SubmitCertificateRequest,
    SubmitCertificateResponse, WatchCertificatesRequest, WatchCertificatesResponse,
};
use topos_core::api::uci::v1::Certificate;

pub const TCE_MOCK_NODE_SOURCE_SUBNET_ID: &str = "5";
const DEFAULT_CHANNEL_STREAM_CAPACITY: usize = 10;

struct TceMockServer;

#[tonic::async_trait]
impl ApiService for TceMockServer {
    type WatchCertificatesStream =
        Pin<Box<dyn Stream<Item = Result<WatchCertificatesResponse, Status>> + Send + 'static>>;

    async fn submit_certificate(
        &self,
        request: Request<SubmitCertificateRequest>,
    ) -> Result<Response<SubmitCertificateResponse>, tonic::Status> {
        let request = request.into_inner();
        println!(
            "TCE MOCK NODE: Certificate submitted to mock tce node: {:?}",
            request
        );
        Ok(Response::new(SubmitCertificateResponse {}))
    }

    async fn watch_certificates(
        &self,
        request: Request<tonic::Streaming<WatchCertificatesRequest>>,
    ) -> Result<Response<Self::WatchCertificatesStream>, tonic::Status> {
        let mut stream: Streaming<_> = request.into_inner();
        println!("TCE node service watch certificates called");
        let (tx, mut rx) =
            mpsc::channel::<WatchCertificatesResponse>(DEFAULT_CHANNEL_STREAM_CAPACITY);

        let output = stream! {
            let mut counter: u32 = 0;
            println!("TCE node service output stream functioning");
            loop {
                println!("TCE node service loop entered counter {}", counter);
                tokio::select! {
                    Some(watch_certificate_request) = stream.next() => {
                        println!("TCE node service processing watch_certificate_request: {:?} counter {}", watch_certificate_request, counter);
                        let mut subnet_ids = vec![];
                        if let Some(watch_certificates_request::Command::OpenStream(open_stream)) = &watch_certificate_request.as_ref().unwrap().command {
                            subnet_ids = open_stream.subnet_ids.clone();
                        };
                        let tx = tx.clone();
                        // Task for generating TCE node service responses
                        tokio::spawn(async move {
                            println!("TCE node service spawned task to send WatchCertificatesResponse");
                            loop {
                                println!("TCE node service generating WatchCertificatesResponse, counter {}", counter);
                                if counter == 0 {
                                    println!("TCE node service preparing WatchCertificatesResponse for StreamOpened call");
                                    // First time send Stream opened message
                                    let _ = tx.send(WatchCertificatesResponse {
                                        request_id: watch_certificate_request.as_ref().unwrap().request_id.clone(),
                                        event: Some(watch_certificates_response::Event::StreamOpened(watch_certificates_response::StreamOpened {subnet_ids: subnet_ids.clone()}))
                                    }).await;
                                } else {
                                    // Other times send certificate
                                    println!("TCE node preparing WatchCertificatesResponse with CertificatePushed");
                                    let _ = tx.send(WatchCertificatesResponse {
                                        request_id: watch_certificate_request.as_ref().unwrap().request_id.clone(),
                                        event: Some(watch_certificates_response::Event::CertificatePushed(watch_certificates_response::CertificatePushed {certificate: Some(Certificate{
                                            initial_subnet_id: TCE_MOCK_NODE_SOURCE_SUBNET_ID.to_string(),
                                            cert_id: counter.to_string(),
                                            prev_cert_id: (counter-1).to_string(),
                                            calls: vec![]
                                        })}))}).await;
                                };
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                counter = counter+1;
                            }
                        });
                    }
                    Some(event) = rx.recv() => {
                        println!("TCE node event received {:?}, yielding to tce outbound stream...", event);
                        yield Ok(event);
                    }
                }
                println!("TCE node sleeping for 2 seconds");
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
    println!("Starting mock TCE node...");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let svc = ApiServiceServer::new(TceMockServer);
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let addr = socket.local_addr().ok().unwrap();

    let service = tokio::spawn(async move {
        Server::builder()
            .add_service(svc)
            .serve_with_shutdown(addr.clone(), shutdown_rx.map(drop))
            .await
            .expect("mock node server successfully started")
    });
    println!("Mock TCE node successfully started");

    Ok((service, shutdown_tx, addr.to_string()))
}
