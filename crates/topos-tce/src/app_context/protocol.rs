use std::collections::HashMap;
use tce_transport::{ProtocolEvents, TceCommands};
use tokio::spawn;
use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_core::uci::SubnetId;
use topos_metrics::CERTIFICATE_DELIVERED_TOTAL;
use topos_tce_storage::errors::{InternalStorageError, StorageError};
use topos_telemetry::PropagationContext;
use tracing::{debug, error, info, info_span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::events::Events;
use crate::messages::NetworkMessage;
use crate::AppContext;

impl AppContext {
    pub async fn on_protocol_event(&mut self, evt: ProtocolEvents) {
        match evt {
            ProtocolEvents::StableSample => {
                info!("Stable Sample detected");
                self.api_client.set_active_sample(true).await;
                if self.events.send(Events::StableSample).await.is_err() {
                    error!("Unable to send StableSample event");
                }
            }

            ProtocolEvents::Broadcast { certificate_id } => {
                info!("Broadcasting certificate {}", certificate_id);
            }

            ProtocolEvents::CertificateDelivered { certificate } => {
                warn!("Certificate delivered {}", certificate.id);
                CERTIFICATE_DELIVERED_TOTAL.inc();
                let storage = self.pending_storage.clone();
                let api_client = self.api_client.clone();

                let certificate_id = certificate.id;
                spawn(async move {
                    match storage
                        .certificate_delivered(certificate_id, Some(certificate.clone()))
                        .await
                    {
                        Ok(positions) => {
                            api_client
                                .dispatch_certificate(
                                    certificate,
                                    positions
                                        .targets
                                        .into_iter()
                                        .map(|(subnet_id, certificate_target_stream_position)| {
                                            (
                                                subnet_id,
                                                TargetStreamPosition {
                                                    target_subnet_id:
                                                        certificate_target_stream_position
                                                            .target_subnet_id,
                                                    source_subnet_id:
                                                        certificate_target_stream_position
                                                            .source_subnet_id,
                                                    position: certificate_target_stream_position
                                                        .position
                                                        .0,
                                                    certificate_id: Some(certificate_id),
                                                },
                                            )
                                        })
                                        .collect::<HashMap<SubnetId, TargetStreamPosition>>(),
                                )
                                .await;
                        }
                        Err(StorageError::InternalStorage(
                            InternalStorageError::CertificateNotFound(_),
                        )) => {
                            error!(
                                "Certificate {} not found in pending storage",
                                certificate_id
                            );
                        }
                        Err(e) => {
                            error!("Pending storage error while delivering certificate: {e}");
                        }
                    };
                });
            }

            ProtocolEvents::Gossip { cert, ctx } => {
                let span = info_span!(
                    parent: &ctx,
                    "SEND Outbound Gossip",
                    peer_id = self.network_client.local_peer_id.to_string(),
                    "otel.kind" = "producer",
                );
                let cert_id = cert.id;

                let data = NetworkMessage::from(TceCommands::OnGossip {
                    cert,
                    ctx: PropagationContext::inject(&span.context()),
                });

                info!("Sending Gossip for certificate {}", cert_id);
                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_GOSSIP, data)
                    .await
                {
                    error!("Unable to send Gossip due to error: {e}");
                }
            }

            ProtocolEvents::Echo {
                certificate_id,
                ctx,
            } => {
                let span = info_span!(
                    parent: &ctx,
                    "SEND Outbound Echo",
                    peer_id = self.network_client.local_peer_id.to_string(),
                    "otel.kind" = "producer",
                );
                // Send echo message
                let data = NetworkMessage::from(TceCommands::OnEcho {
                    certificate_id,
                    ctx: PropagationContext::inject(&span.context()),
                });

                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_ECHO, data)
                    .await
                {
                    error!("Unable to send Echo due to error: {e}");
                }
            }

            ProtocolEvents::Ready {
                certificate_id,
                ctx,
            } => {
                let span = info_span!(
                    parent: &ctx,
                    "SEND Outbound Ready",
                    peer_id = self.network_client.local_peer_id.to_string(),
                    "otel.kind" = "producer",
                );
                let data = NetworkMessage::from(TceCommands::OnReady {
                    certificate_id,
                    ctx: PropagationContext::inject(&span.context()),
                });

                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_READY, data)
                    .await
                {
                    error!("Unable to send Ready due to error: {e}");
                }
            }

            evt => {
                debug!("Unhandled event: {:?}", evt);
            }
        }
    }
}
