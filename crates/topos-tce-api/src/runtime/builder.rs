use futures::Stream;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    spawn,
    sync::{broadcast, mpsc, oneshot, RwLock},
};
use tokio_stream::wrappers::ReceiverStream;
use topos_core::api::grpc::tce::v1::StatusResponse;
use topos_tce_storage::{
    fullnode::FullNodeStore, types::CertificateDeliveredWithPositions, StorageClient,
};

use crate::{
    graphql::builder::ServerBuilder as GraphQLBuilder, grpc::builder::ServerBuilder,
    metrics::builder::ServerBuilder as MetricsBuilder, Runtime, RuntimeClient, RuntimeEvent,
};

#[derive(Default)]
pub struct RuntimeBuilder {
    storage: Option<StorageClient>,
    store: Option<Arc<FullNodeStore>>,
    broadcast_stream: Option<broadcast::Receiver<CertificateDeliveredWithPositions>>,
    local_peer_id: String,
    grpc_socket_addr: Option<SocketAddr>,
    graphql_socket_addr: Option<SocketAddr>,
    metrics_socket_addr: Option<SocketAddr>,
    status: Option<RwLock<StatusResponse>>,
}

impl RuntimeBuilder {
    pub fn with_broadcast_stream(
        mut self,
        stream: broadcast::Receiver<CertificateDeliveredWithPositions>,
    ) -> Self {
        self.broadcast_stream = Some(stream);

        self
    }

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

    pub fn store(mut self, store: Arc<FullNodeStore>) -> Self {
        self.store = Some(store);

        self
    }

    pub fn storage(mut self, storage: StorageClient) -> Self {
        self.storage = Some(storage);

        self
    }

    pub async fn build_and_launch(
        mut self,
    ) -> (
        RuntimeClient,
        impl Stream<Item = RuntimeEvent>,
        RuntimeContext,
    ) {
        let (command_sender, internal_runtime_command_receiver) = mpsc::channel(2048);
        let (api_event_sender, api_event_receiver) = mpsc::channel(2048);

        let (health_reporter, tce_status, grpc) = ServerBuilder::default()
            .with_peer_id(self.local_peer_id)
            .command_sender(command_sender.clone())
            .serve_addr(self.grpc_socket_addr)
            .build()
            .await;

        let (command_sender, runtime_command_receiver) = mpsc::channel(2048);
        let (shutdown_channel, shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);

        let grpc_handler = spawn(grpc);

        let graphql_handler = if let Some(graphql_addr) = self.graphql_socket_addr {
            tracing::info!("Serving GraphQL on {}", graphql_addr);

            let graphql = GraphQLBuilder::default()
                .store(
                    self.store
                        .take()
                        .expect("Unable to build GraphQL Server, Store is missing"),
                )
                .serve_addr(Some(graphql_addr))
                .build();
            spawn(graphql.await)
        } else {
            spawn(async move {
                tracing::info!("Not serving GraphQL");
                Ok(())
            })
        };

        let metrics_handler = if let Some(metrics_addr) = self.metrics_socket_addr {
            tracing::info!("Serving metrics on {}", metrics_addr);

            let metrics_server = MetricsBuilder::default()
                .serve_addr(Some(metrics_addr))
                .build();
            spawn(metrics_server.await)
        } else {
            spawn(async move {
                tracing::info!("Not serving metrics");
                Ok(())
            })
        };

        let runtime = Runtime {
            sync_tasks: HashMap::new(),
            broadcast_stream: self
                .broadcast_stream
                .expect("Unable to build Runtime, Broadcast Stream is missing"),
            storage: self
                .storage
                .take()
                .expect("Unable to build Runtime, Storage is missing"),
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
        let runtime_handler = spawn(runtime.launch());

        (
            RuntimeClient {
                command_sender,
                tce_status,
                shutdown_channel,
            },
            ReceiverStream::new(api_event_receiver),
            RuntimeContext {
                grpc_handler,
                graphql_handler,
                metrics_handler,
                runtime_handler,
            },
        )
    }

    pub fn set_grpc_socket_addr(mut self, socket: Option<SocketAddr>) -> Self {
        self.grpc_socket_addr = socket;

        self
    }
}

#[derive(Debug)]
pub struct RuntimeContext {
    grpc_handler: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    graphql_handler: tokio::task::JoinHandle<Result<(), hyper::Error>>,
    metrics_handler: tokio::task::JoinHandle<Result<(), hyper::Error>>,
    runtime_handler: tokio::task::JoinHandle<()>,
}

impl Drop for RuntimeContext {
    fn drop(&mut self) {
        tracing::warn!("Dropping RuntimeContext");
        self.grpc_handler.abort();
        self.graphql_handler.abort();
        self.metrics_handler.abort();
        self.runtime_handler.abort();
    }
}
