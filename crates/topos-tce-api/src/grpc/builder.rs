use std::{net::SocketAddr, sync::Arc};

use futures::{future::BoxFuture, FutureExt};
use tokio::sync::{mpsc::Sender, RwLock};
use tonic_health::server::HealthReporter;
use topos_core::api::tce::v1::{
    api_service_server::ApiServiceServer, console_service_server::ConsoleServiceServer,
    StatusResponse,
};

use crate::runtime::InternalRuntimeCommand;

use super::{console::TceConsoleService, TceGrpcService};

#[derive(Debug, Default)]
pub struct ServerBuilder {
    local_peer_id: String,
    command_sender: Option<Sender<InternalRuntimeCommand>>,
    serve_addr: Option<SocketAddr>,
}

impl ServerBuilder {
    pub(crate) fn with_peer_id(mut self, local_peer_id: String) -> Self {
        self.local_peer_id = local_peer_id;

        self
    }

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
        Arc<RwLock<StatusResponse>>,
        BoxFuture<'static, Result<(), tonic::transport::Error>>,
    ) {
        let local_peer_id = self.local_peer_id;
        let command_sender = self
            .command_sender
            .take()
            .expect("Cannot build gRPC without an InternalRuntimeCommand sender");

        let status = Arc::new(RwLock::new(StatusResponse {
            has_active_sample: false,
        }));

        let console = ConsoleServiceServer::new(TceConsoleService {
            command_sender: command_sender.clone(),
            status: status.clone(),
        });

        let service = ApiServiceServer::new(TceGrpcService {
            local_peer_id,
            command_sender,
        });

        let (mut health_reporter, health_service) = tonic_health::server::health_reporter();

        health_reporter
            .set_serving::<ApiServiceServer<TceGrpcService>>()
            .await;

        let reflexion = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(topos_core::api::FILE_DESCRIPTOR_SET)
            .build()
            .expect("Cannot build gRPC because of FILE_DESCRIPTOR_SET error");

        let serve_addr = self
            .serve_addr
            .take()
            .expect("Cannot build gRPC without a valid serve_addr");

        let grpc = tonic::transport::Server::builder()
            .add_service(health_service)
            .add_service(service)
            .add_service(console)
            .add_service(reflexion)
            .serve(serve_addr)
            .boxed();

        (health_reporter, status, grpc)
    }
}
