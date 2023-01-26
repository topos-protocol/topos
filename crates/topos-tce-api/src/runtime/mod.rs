use opentelemetry::{
    trace::{FutureExt, TraceContextExt},
    Context,
};
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
use topos_core::uci::SubnetId;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use tracing::{debug, error, info, info_span, Instrument, Span};
use uuid::Uuid;

use crate::{
    grpc::TceGrpcService,
    stream::{Stream, StreamCommand},
};

pub(crate) mod builder;
mod client;
mod commands;
pub mod error;
mod events;

#[cfg(test)]
mod tests;

pub use client::RuntimeClient;

use self::builder::RuntimeBuilder;
pub(crate) use self::commands::InternalRuntimeCommand;

pub use self::commands::RuntimeCommand;
pub use self::events::RuntimeEvent;

pub struct Runtime {
    /// Streams that are currently active (with a valid handshake)
    pub(crate) active_streams: HashMap<Uuid, Sender<StreamCommand>>,
    /// Streams that are currently in negotiation
    pub(crate) pending_streams: HashMap<Uuid, Sender<StreamCommand>>,
    /// Mapping between a subnet_id and streams that are subscribed to it
    pub(crate) subnet_subscription: HashMap<SubnetId, HashSet<Uuid>>,
    /// Receiver for Internal API command
    pub(crate) internal_runtime_command_receiver: Receiver<InternalRuntimeCommand>,
    /// Receiver for Outside API command
    pub(crate) runtime_command_receiver: Receiver<RuntimeCommand>,
    /// HealthCheck reporter for gRPC
    pub(crate) health_reporter: HealthReporter,
    /// Sender that forward Event to the rest of the system
    pub(crate) api_event_sender: Sender<RuntimeEvent>,
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::default()
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
            RuntimeCommand::DispatchCertificate { certificate } => {
                info!(
                    "Received DispatchCertificate for certificate cert_id: {:?}",
                    certificate.id
                );
                // Collect target subnets from certificate cross chain transaction list
                let target_subnets = certificate.target_subnets.iter().collect::<HashSet<_>>();
                debug!(
                    "Dispatching certificate cert_id: {:?} to target subnets: {:?}",
                    &certificate.id, target_subnets
                );
                for target_subnet_id in target_subnets {
                    if let Some(stream_list) = self.subnet_subscription.get(target_subnet_id) {
                        let uuids: Vec<&Uuid> = stream_list.iter().collect();
                        for uuid in uuids {
                            if let Some(sender) = self.active_streams.get(uuid) {
                                let sender = sender.clone();
                                // TODO: Switch to arc
                                let certificate = certificate.clone();

                                info!("Sending certificate to {uuid}");
                                spawn(async move {
                                    if let Err(error) = sender
                                        .send(StreamCommand::PushCertificate { certificate })
                                        .await
                                    {
                                        error!(%error, "Can't push certificate because receiver is dropped");
                                    }
                                });
                            }
                        }
                    }
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
                        .entry(subnet_id.into())
                        .or_default()
                        .insert(stream_id);
                }

                if let Err(error) = sender.send(Ok(())) {
                    error!(?error, "Can't send response to Stream, receiver is dropped");
                }
            }

            InternalRuntimeCommand::CertificateSubmitted {
                certificate,
                sender,
                ctx,
            } => {
                let span = info_span!(parent: &ctx, "TCE API Runtime",);

                async move {
                    tracing::warn!(span_span_id = ?Span::current().context().span().span_context().span_id());
                    tracing::warn!(cx_span_id = ?Context::current().span().span_context().span_id());

                    info!(
                        "A certificate has been submitted to the TCE {}",
                        certificate.id
                    );
                    if let Err(error) = self
                        .api_event_sender
                        .send(RuntimeEvent::CertificateSubmitted {
                            certificate,
                            sender,
                            ctx: Span::current().clone(),
                        })
                        .with_current_context()
                        .instrument(Span::current())
                        .await
                    {
                        error!(
                            %error,
                            "Can't send certificate submission to runtime, receiver is dropped"
                        );
                    }
                }
                .with_context(span.context())
                .instrument(span)
                .await
            }

            InternalRuntimeCommand::PushPeerList { peers, sender } => {
                // debug!("A peer list has been pushed {:?}", peers);

                if let Err(error) = self
                    .api_event_sender
                    .send(RuntimeEvent::PeerListPushed { peers, sender })
                    .await
                {
                    error!(
                        %error,
                        "Can't send new peer list to runtime, receiver is dropped"
                    );
                }
            }

            InternalRuntimeCommand::GetSourceHead { subnet_id, sender } => {
                info!(
                    "Source head certificate has been requested for subnet id: {:?}",
                    subnet_id
                );

                if let Err(error) = self
                    .api_event_sender
                    .send(RuntimeEvent::GetSourceHead { subnet_id, sender })
                    .await
                {
                    error!(
                        %error,
                        "Can't request source head certificate, receiver is dropped"
                    );
                }
            }
        }
    }
}
