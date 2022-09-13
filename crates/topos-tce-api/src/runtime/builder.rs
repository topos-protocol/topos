use futures::future::BoxFuture;
use std::collections::HashMap;
use tokio::{spawn, sync::mpsc};

use crate::{grpc::builder::ServerBuilder, Runtime, RuntimeClient};

#[derive(Default)]
pub struct RuntimeBuilder {}

impl RuntimeBuilder {
    pub async fn build(self) -> (RuntimeClient, RuntimeLauncher) {
        let (command_sender, internal_runtime_command_receiver) = mpsc::channel(2048);

        let (health_reporter, grpc) = ServerBuilder::default()
            .command_sender(command_sender)
            .build()
            .await;

        let (command_sender, runtime_command_receiver) = mpsc::channel(2048);

        let runtime = Runtime {
            active_streams: HashMap::new(),
            pending_streams: HashMap::new(),
            subnet_subscription: HashMap::new(),
            internal_runtime_command_receiver,
            runtime_command_receiver,
            health_reporter,
        };

        (
            RuntimeClient { command_sender },
            RuntimeLauncher { runtime, grpc },
        )
    }
}

pub struct RuntimeLauncher {
    pub(crate) grpc: BoxFuture<'static, Result<(), tonic::transport::Error>>,
    pub(crate) runtime: Runtime,
}

impl RuntimeLauncher {
    pub async fn launch(self) {
        // launch gRPC
        spawn(self.grpc);
        spawn(self.runtime.launch());
    }
}
