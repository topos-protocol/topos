use futures::Stream;
use std::{collections::HashMap, net::SocketAddr};
use tokio::{
    spawn,
    sync::{mpsc, oneshot, RwLock},
};
use tokio_stream::wrappers::ReceiverStream;
use topos_core::api::grpc::tce::v1::StatusResponse;
use topos_tce_storage::StorageClient;

use crate::{
    graphql::builder::ServerBuilder as GraphQLBuilder, grpc::builder::ServerBuilder,
    metrics::builder::ServerBuilder as MetricsBuilder, Runtime, RuntimeClient, RuntimeEvent,
};

#[derive(Default)]
pub struct RuntimeBuilder {
    storage: Option<StorageClient>,
    local_peer_id: String,
    grpc_socket_addr: Option<SocketAddr>,
    graphql_socket_addr: Option<SocketAddr>,
    metrics_socket_addr: Option<SocketAddr>,
    status: Option<RwLock<StatusResponse>>,
}

impl RuntimeBuilder {
    pub fn with_peer_id(mut self, local_peer_id: String) -> Self {
        self.local_peer_id = local_peer_id;

        self
    }

    pub fn serve_grpc_addr(mut self, addr: SocketAddr) -> Self {
        self.grpc_socket_addr = Some(addr);

        self
    }

    pub fn serve_graphql_addr(mut self, addr: SocketAddr) -> Self {
        self.graphql_socket_addr = Some(addr);

        self
    }

    pub fn serve_metrics_addr(mut self, addr: SocketAddr) -> Self {
        self.metrics_socket_addr = Some(addr);

        self
    }

    pub fn tce_status(mut self, status: RwLock<StatusResponse>) -> Self {
        self.status = Some(status);

        self
    }

    pub fn storage(mut self, storage: StorageClient) -> Self {
        self.storage = Some(storage);

        self
    }

    pub async fn build_and_launch(mut self) -> (RuntimeClient, impl Stream<Item = RuntimeEvent>) {
        let (command_sender, internal_runtime_command_receiver) = mpsc::channel(2048);
        let (api_event_sender, api_event_receiver) = mpsc::channel(2048);

        let (health_reporter, tce_status, grpc) = ServerBuilder::default()
            .with_peer_id(self.local_peer_id)
            .command_sender(command_sender.clone())
            .serve_addr(self.grpc_socket_addr)
            .build()
            .await;

        let graphql = GraphQLBuilder::default()
            .storage(self.storage.clone())
            .serve_addr(self.graphql_socket_addr)
            .build();

        let metrics_server = MetricsBuilder::default()
            .serve_addr(self.metrics_socket_addr)
            .build();

        let (command_sender, runtime_command_receiver) = mpsc::channel(2048);
        let (shutdown_channel, shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);

        let runtime = Runtime {
            sync_tasks: HashMap::new(),
            // TODO: remove unwrap
            storage: self.storage.take().unwrap(),
            active_streams: HashMap::new(),
            pending_streams: HashMap::new(),
            subnet_subscriptions: HashMap::new(),
            internal_runtime_command_receiver,
            runtime_command_receiver,
            health_reporter,
            api_event_sender,
            shutdown: shutdown_receiver,
            streams: Default::default(),
        };

        spawn(grpc);
        spawn(graphql.await);
        spawn(metrics_server.await);
        spawn(runtime.launch());

        (
            RuntimeClient {
                command_sender,
                tce_status,
                shutdown_channel,
            },
            ReceiverStream::new(api_event_receiver),
        )
    }

    pub fn set_grpc_socket_addr(mut self, socket: Option<SocketAddr>) -> Self {
        self.grpc_socket_addr = socket;

        self
    }
}
