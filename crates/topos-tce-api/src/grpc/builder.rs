use std::net::SocketAddr;

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::mpsc::Sender;
use tonic_health::server::HealthReporter;
use topos_core::api::tce::v1::api_service_server::ApiServiceServer;

use crate::runtime::InternalRuntimeCommand;

use super::TceGrpcService;

const DEFAULT_SOCKET_ADDR: &str = "127.0.0.1:1340";

#[derive(Debug, Default)]
pub struct ServerBuilder {
    command_sender: Option<Sender<InternalRuntimeCommand>>,
    serve_addr: Option<SocketAddr>,
}

impl ServerBuilder {
    pub(crate) fn command_sender(mut self, sender: Sender<InternalRuntimeCommand>) -> Self {
        self.command_sender = Some(sender);

        self
    }

    pub(crate) fn serve_addr(mut self, addr: Option<SocketAddr>) -> Self {
        self.serve_addr = addr;

        self
    }

    pub async fn build(
        mut self,
    ) -> (
        HealthReporter,
        BoxFuture<'static, Result<(), tonic::transport::Error>>,
    ) {
        let command_sender = self
            .command_sender
            .take()
            .expect("Cannot build gRPC without an InternalRuntimeCommand sender");

        let service = ApiServiceServer::new(TceGrpcService { command_sender });

        let (mut health_reporter, health_service) = tonic_health::server::health_reporter();

        health_reporter
            .set_serving::<ApiServiceServer<TceGrpcService>>()
            .await;

        let reflexion = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(topos_core::api::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        let serve_addr = if let Some(addr) = self.serve_addr {
            addr
        } else {
            // TODO: Remove this when we'll have grpc addr validation on CLI side.
            DEFAULT_SOCKET_ADDR.parse().unwrap()
        };

        let grpc = tonic::transport::Server::builder()
            .add_service(health_service)
            .add_service(service)
            .add_service(reflexion)
            .serve(serve_addr)
            .boxed();

        (health_reporter, grpc)
    }
}
