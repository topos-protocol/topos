use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::{spawn, sync::oneshot};
use tonic::{Status, Streaming};
use topos_core::api::tce::v1::{WatchCertificatesRequest, WatchCertificatesResponse};
use tracing::info;
use uuid::Uuid;

mod grpc;
use grpc::ServerBuilder;

mod stream;
use stream::{Stream, StreamCommand};

pub struct Runtime {
    active_streams: HashMap<Uuid, Sender<StreamCommand>>,
    pending_streams: HashMap<Uuid, Sender<StreamCommand>>,
    subnet_subscription: HashMap<String, HashSet<Uuid>>,
    internal_runtime_command_receiver: mpsc::Receiver<InternalRuntimeCommand>,
}

impl Runtime {
    pub fn build() -> ServerBuilder {
        ServerBuilder::default()
    }

    pub async fn launch(mut self) {
        loop {
            tokio::select! {
                Some(internal_command) = self.internal_runtime_command_receiver.recv() => {
                    self.handle_command(internal_command).await;

                }
            }
        }
    }

    async fn handle_command(&mut self, command: InternalRuntimeCommand) {
        match command {
            InternalRuntimeCommand::NewStream {
                stream,
                sender,
                internal_runtime_command_sender,
            } => {
                let stream_id = Uuid::new_v4();
                info!("Opening a new stream with UUID {stream_id}");

                let (command_sender, command_receiver) = mpsc::channel(2048);

                self.pending_streams.insert(stream_id, command_sender);

                let active_stream = Stream {
                    stream_id,
                    stream,
                    sender,
                    command_receiver,
                    internal_runtime_command_sender,
                };

                spawn(active_stream.run());
            }

            InternalRuntimeCommand::StreamTimeout { stream_id } => {
                self.pending_streams.remove(&stream_id);
            }

            InternalRuntimeCommand::Handshaked { stream_id } => {
                if let Some(sender) = self.pending_streams.remove(&stream_id) {
                    self.active_streams.insert(stream_id, sender);
                }
            }

            InternalRuntimeCommand::Register {
                stream_id,
                subnet_id,
                sender,
            } => {
                self.subnet_subscription
                    .entry(subnet_id)
                    .or_default()
                    .insert(stream_id);

                _ = sender.send(Ok(()));
            }
        }
    }
}

enum RuntimeEvent {}
enum InternalRuntimeEvent {}

enum RuntimeCommand {}
#[derive(Debug)]
enum InternalRuntimeCommand {
    NewStream {
        stream: Streaming<WatchCertificatesRequest>,
        sender: mpsc::Sender<Result<WatchCertificatesResponse, Status>>,
        internal_runtime_command_sender: Sender<InternalRuntimeCommand>,
    },

    Register {
        stream_id: Uuid,
        subnet_id: String,
        sender: oneshot::Sender<Result<(), ()>>,
    },

    StreamTimeout {
        stream_id: Uuid,
    },

    Handshaked {
        stream_id: Uuid,
    },
}
