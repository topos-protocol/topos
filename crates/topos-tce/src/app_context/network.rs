use opentelemetry::trace::{FutureExt as TraceFutureExt, TraceContextExt};
use tce_transport::TceCommands;
use tokio::spawn;
use topos_p2p::Event as NetEvent;
use topos_tce_broadcast::DoubleEchoCommand;
use tracing::{error, info, info_span, trace, Instrument};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::messages::NetworkMessage;
use crate::AppContext;

impl AppContext {
    pub async fn on_net_event(&mut self, evt: NetEvent) {
        trace!(
            "on_net_event: peer: {} event {:?}",
            &self.network_client.local_peer_id,
            &evt
        );

        if let NetEvent::Gossip { from, data } = evt {
            let msg: NetworkMessage = data.into();

            if let NetworkMessage::Cmd(cmd) = msg {
                match cmd {
                    TceCommands::OnGossip { cert, ctx } => {
                        let span = info_span!(
                            "RECV Outbound Gossip",
                            peer_id = self.network_client.local_peer_id.to_string(),
                            "otel.kind" = "consumer",
                            sender = from.to_string()
                        );
                        let parent = ctx.extract();
                        span.add_link(parent.span().span_context().clone());

                        let channel = self.tce_cli.get_double_echo_channel();

                        spawn(async move {
                            info!("Send certificate to be broadcast");
                            if channel
                                .send(DoubleEchoCommand::Broadcast {
                                    cert,
                                    need_gossip: false,
                                    ctx: span,
                                })
                                .await
                                .is_err()
                            {
                                error!("Unable to send broadcast_new_certificate command, Receiver was dropped");
                            }
                        });
                    }

                    TceCommands::OnEcho {
                        certificate_id,
                        ctx,
                    } => {
                        let span = info_span!(
                            "RECV Outbound Echo",
                            peer_id = self.network_client.local_peer_id.to_string(),
                            "otel.kind" = "consumer",
                            sender = from.to_string()
                        );
                        let context = ctx.extract();
                        span.add_link(context.span().span_context().clone());

                        let channel = self.tce_cli.get_double_echo_channel();
                        spawn(async move {
                            if let Err(e) = channel
                                .send(DoubleEchoCommand::Echo {
                                    from_peer: from,
                                    certificate_id,
                                    ctx: span.clone(),
                                })
                                .with_context(span.context().clone())
                                .instrument(span)
                                .await
                            {
                                error!("Unable to send Echo, {:?}", e);
                            }
                        });
                    }
                    TceCommands::OnReady {
                        certificate_id,
                        ctx,
                    } => {
                        let span = info_span!(
                            "RECV Outbound Ready",
                            peer_id = self.network_client.local_peer_id.to_string(),
                            "otel.kind" = "consumer",
                            sender = from.to_string()
                        );
                        let context = ctx.extract();
                        span.add_link(context.span().span_context().clone());

                        let channel = self.tce_cli.get_double_echo_channel();
                        spawn(async move {
                            if let Err(e) = channel
                                .send(DoubleEchoCommand::Ready {
                                    from_peer: from,
                                    certificate_id,
                                    ctx: span.clone(),
                                })
                                .with_context(context)
                                .instrument(span)
                                .await
                            {
                                error!("Unable to send Ready {:?}", e);
                            }
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}
