use futures::Stream as FutureStream;
use std::collections::HashMap;
use std::pin::Pin;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use topos_core::api::tce::v1::{
    api_service_server::{ApiService, ApiServiceServer},
    SubmitCertificateRequest, SubmitCertificateResponse, WatchCertificatesRequest,
    WatchCertificatesResponse,
};

use crate::runtime::{InternalRuntimeCommand, Runtime, RuntimeClient};

const DEFAULT_CHANNEL_STREAM_CAPACITY: usize = 100;

#[derive(Debug)]
struct TceGrpcService {
    command_sender: mpsc::Sender<InternalRuntimeCommand>,
}

#[derive(Debug, Default)]
pub struct ServerBuilder {
    service: Option<ApiServiceServer<TceGrpcService>>,
    runtime: Option<Runtime>,
}

impl ServerBuilder {
    pub fn build(mut self) -> (RuntimeClient, Self) {
        let (command_sender, internal_runtime_command_receiver) = mpsc::channel(2048);

        self.service = Some(ApiServiceServer::new(TceGrpcService { command_sender }));

        let (command_sender, runtime_command_receiver) = mpsc::channel(2048);

        self.runtime = Some(Runtime {
            active_streams: HashMap::new(),
            pending_streams: HashMap::new(),
            subnet_subscription: HashMap::new(),
            internal_runtime_command_receiver,
            runtime_command_receiver,
        });

        (RuntimeClient { command_sender }, self)
    }

    pub async fn launch(mut self) {
        let service = self
            .service
            .take()
            .expect("Unable to start because gRPC service is not defined");

        let runtime = self
            .runtime
            .take()
            .expect("Unable to start because API Runtime is not defined");

        spawn(runtime.launch());

        let reflexion = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(topos_core::api::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        _ = tonic::transport::Server::builder()
            .add_service(service)
            .add_service(reflexion)
            .serve("127.0.0.1:1340".parse().unwrap())
            .await;
    }
}

#[tonic::async_trait]
impl ApiService for TceGrpcService {
    async fn submit_certificate(
        &self,
        _request: Request<SubmitCertificateRequest>,
    ) -> Result<Response<SubmitCertificateResponse>, Status> {
        Err(Status::unimplemented(""))
    }

    ///Server streaming response type for the WatchCertificates method.
    type WatchCertificatesStream = Pin<
        Box<dyn FutureStream<Item = Result<WatchCertificatesResponse, Status>> + Send + 'static>,
    >;

    /// This RPC allows a client to open a bidirectional stream with a TCE
    async fn watch_certificates(
        &self,
        request: Request<Streaming<WatchCertificatesRequest>>,
    ) -> Result<Response<Self::WatchCertificatesStream>, Status> {
        let stream: Streaming<_> = request.into_inner();

        let (sender, rx) = mpsc::channel::<Result<WatchCertificatesResponse, Status>>(
            DEFAULT_CHANNEL_STREAM_CAPACITY,
        );

        _ = self
            .command_sender
            .send(InternalRuntimeCommand::NewStream {
                stream,
                sender,
                internal_runtime_command_sender: self.command_sender.clone(),
            })
            .await;

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::WatchCertificatesStream
        ))
    }
}
