use futures::Stream;
use std::{collections::HashMap, net::SocketAddr};
use tokio::{
    spawn,
    sync::{mpsc, oneshot, RwLock},
};
use tokio_stream::wrappers::ReceiverStream;
use topos_core::api::tce::v1::StatusResponse;

use crate::{grpc::builder::ServerBuilder, Runtime, RuntimeClient, RuntimeEvent};

#[derive(Default)]
pub struct RuntimeBuilder {
    local_peer_id: String,
    grpc_socket_addr: Option<SocketAddr>,
    status: Option<RwLock<StatusResponse>>,
}

impl RuntimeBuilder {
    pub fn with_peer_id(mut self, local_peer_id: String) -> Self {
        self.local_peer_id = local_peer_id;

        self
    }

    pub fn serve_addr(mut self, addr: SocketAddr) -> Self {
        self.grpc_socket_addr = Some(addr);

        self
    }

    pub fn tce_status(mut self, status: RwLock<StatusResponse>) -> Self {
        self.status = Some(status);

        self
    }

    pub async fn build_and_launch(self) -> (RuntimeClient, impl Stream<Item = RuntimeEvent>) {
        let (command_sender, internal_runtime_command_receiver) = mpsc::channel(2048);
        let (api_event_sender, api_event_receiver) = mpsc::channel(2048);

        let (health_reporter, tce_status, grpc) = ServerBuilder::default()
            .with_peer_id(self.local_peer_id)
            .command_sender(command_sender)
            .serve_addr(self.grpc_socket_addr)
            .build()
            .await;

        let (command_sender, runtime_command_receiver) = mpsc::channel(2048);
        let (shutdown_channel, shutdown_receiver) = mpsc::channel::<oneshot::Sender<()>>(1);

        let runtime = Runtime {
            active_streams: HashMap::new(),
            pending_streams: HashMap::new(),
            subnet_subscription: HashMap::new(),
            internal_runtime_command_receiver,
            runtime_command_receiver,
            health_reporter,
            api_event_sender,
            shutdown: shutdown_receiver,
        };

        spawn(grpc);
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
