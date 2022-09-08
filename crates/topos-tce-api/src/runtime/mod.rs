use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use tokio::{
    spawn,
    sync::mpsc::{self, Receiver, Sender},
};
use tonic_health::server::HealthReporter;
use topos_core::api::tce::v1::api_service_server::ApiServiceServer;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    grpc::{ServerBuilder, TceGrpcService},
    stream::{Stream, StreamCommand},
};

mod client;
mod commands;
mod events;

#[cfg(test)]
mod tests;

pub use client::RuntimeClient;

pub(crate) use self::commands::InternalRuntimeCommand;

pub use self::commands::RuntimeCommand;
pub use self::events::RuntimeEvent;

#[derive(Debug)]
pub struct Runtime {
    pub(crate) active_streams: HashMap<Uuid, Sender<StreamCommand>>,
    pub(crate) pending_streams: HashMap<Uuid, Sender<StreamCommand>>,
    pub(crate) subnet_subscription: HashMap<String, HashSet<Uuid>>,
    pub(crate) internal_runtime_command_receiver: Receiver<InternalRuntimeCommand>,
    pub(crate) runtime_command_receiver: Receiver<RuntimeCommand>,
    pub(crate) health_reporter: HealthReporter,
}

impl Runtime {
    pub fn builder() -> ServerBuilder {
        ServerBuilder::default()
    }

    pub async fn launch(mut self) {
        let mut health_update = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = health_update.tick() => {
                    self.health_reporter.set_serving::<ApiServiceServer<TceGrpcService>>().await;
                }
                Some(internal_command) = self.internal_runtime_command_receiver.recv() => {
                    self.handle_internal_command(internal_command).await;
                }

                Some(command) = self.runtime_command_receiver.recv() => {
                    self.handle_runtime_command(command).await;
                }
            }
        }
    }

    async fn handle_runtime_command(&mut self, command: RuntimeCommand) {
        match command {
            RuntimeCommand::DispatchCertificate {
                subnet_id,
                certificate,
            } => {
                info!("Received DispatchCertificate");
                if let Some(stream_list) = self.subnet_subscription.get(&subnet_id) {
                    let uuids: Vec<&Uuid> = stream_list.iter().collect();

                    for uuid in uuids {
                        if let Some(sender) = self.active_streams.get(uuid) {
                            let sender = sender.clone();
                            // TODO: Switch to arc
                            let subnet_id = subnet_id.clone();
                            let certificate = certificate.clone();

                            info!("Sending certificate to {uuid}");
                            spawn(async move {
                                _ = sender
                                    .send(StreamCommand::PushCertificate {
                                        subnet_id,
                                        certificate,
                                    })
                                    .await;
                            });
                        }
                    }
                } else {
                    error!("No subscription for this subnet");
                }
            }
        }
    }

    async fn handle_internal_command(&mut self, command: InternalRuntimeCommand) {
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
                    info!("{stream_id} has successfully handshake");
                }
            }

            InternalRuntimeCommand::Register {
                stream_id,
                subnet_ids,
                sender,
            } => {
                info!("{stream_id} is registered as subscriber for {subnet_ids:?}");
                for subnet_id in subnet_ids {
                    self.subnet_subscription
                        .entry(subnet_id.value)
                        .or_default()
                        .insert(stream_id);
                }

                _ = sender.send(Ok(()));
            }

            InternalRuntimeCommand::CertificateSubmitted { certificate } => {
                info!("A certificate has been submitted to the TCE {certificate:?}");
            }
        }
    }
}
