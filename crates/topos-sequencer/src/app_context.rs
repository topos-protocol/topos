//!
//! Application logic glue
//!
use crate::SequencerConfiguration;
use opentelemetry::trace::FutureExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use topos_sequencer_subnet_runtime::proxy::{SubnetRuntimeProxyCommand, SubnetRuntimeProxyEvent};
use topos_sequencer_subnet_runtime::SubnetRuntimeProxyWorker;
use topos_tce_proxy::{worker::TceProxyWorker, TceProxyCommand, TceProxyEvent};
use tracing::{debug, error, info, info_span, warn, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Top-level transducer sequencer app context & driver (alike)
///
/// Implements <...Host> traits for network and Api, listens for protocol events in events
/// (store is not active component).
///
/// In the end we shall come to design where this struct receives
/// config+data as input and runs app returning data as output
///
pub struct AppContext {
    pub config: SequencerConfiguration,
    pub subnet_runtime_proxy_worker: SubnetRuntimeProxyWorker,
    pub tce_proxy_worker: TceProxyWorker,
}

impl AppContext {
    /// Factory
    pub fn new(
        config: SequencerConfiguration,
        runtime_proxy_worker: SubnetRuntimeProxyWorker,
        tce_proxy_worker: TceProxyWorker,
    ) -> Self {
        Self {
            config,
            subnet_runtime_proxy_worker: runtime_proxy_worker,
            tce_proxy_worker,
        }
    }

    /// Main processing loop
    pub(crate) async fn run(&mut self, shutdown: (CancellationToken, mpsc::Sender<()>)) {
        loop {
            tokio::select! {

                // Subnet event handling
                Ok(evt) = self.subnet_runtime_proxy_worker.next_event() => {
                    debug!("runtime_proxy_worker.next_event(): {:?}", &evt);
                    self.on_subnet_runtime_proxy_event(evt).await;
                },


                // TCE event handling
                Ok(tce_evt) = self.tce_proxy_worker.next_event() => {
                    debug!("tce_proxy_worker.next_event(): {:?}", &tce_evt);
                    self.on_tce_proxy_event(tce_evt).await;
                },

                // Shutdown signal
                _ = shutdown.0.cancelled() => {
                    info!("Shutting down Sequencer app context...");
                    if let Err(e) = self.shutdown().await {
                        error!("Error shutting down Sequencer app context: {e}");
                    }
                    // Drop the sender to notify the Sequencer termination
                    drop(shutdown.1);
                    break;
                }
            }
        }
    }

    async fn on_subnet_runtime_proxy_event(&mut self, evt: SubnetRuntimeProxyEvent) {
        debug!("on_subnet_runtime_proxy_event : {:?}", &evt);
        match evt {
            SubnetRuntimeProxyEvent::NewCertificate { cert, ctx } => {
                let span = info_span!("Sequencer app context");
                span.set_parent(ctx);
                if let Err(e) = self
                    .tce_proxy_worker
                    .send_command(TceProxyCommand::SubmitCertificate {
                        cert,
                        ctx: span.context(),
                    })
                    .with_context(span.context())
                    .instrument(span)
                    .await
                {
                    error!("Unable to send tce proxy command {e}");
                }
            }
            SubnetRuntimeProxyEvent::NewEra(_authorities) => {
                todo!()
            }
        }
    }

    async fn on_tce_proxy_event(&mut self, evt: TceProxyEvent) {
        match evt {
            TceProxyEvent::NewDeliveredCerts { certificates, ctx } => {
                let span = info_span!("Sequencer app context");
                span.set_parent(ctx);

                async {
                    // New certificates acquired from TCE
                    for (cert, cert_position) in certificates {
                        self.subnet_runtime_proxy_worker
                            .eval(SubnetRuntimeProxyCommand::OnNewDeliveredCertificate {
                                certificate: cert,
                                position: cert_position,
                                ctx: Span::current().context(),
                            })
                            .await
                            .expect("Propagate new delivered Certificate to the runtime");
                    }
                }
                .with_context(span.context())
                .instrument(span)
                .await
            }
            TceProxyEvent::WatchCertificatesChannelFailed => {
                warn!("Restarting tce proxy worker...");
                let config = &self.tce_proxy_worker.config;
                // Here try to restart tce proxy
                _ = self.tce_proxy_worker.shutdown().await;

                // TODO: Retrieve subnet checkpoint from where to start receiving certificates, again
                let (tce_proxy_worker, _source_head_certificate_id) = match TceProxyWorker::new(
                    topos_tce_proxy::TceProxyConfig {
                        subnet_id: config.subnet_id,
                        base_tce_api_url: config.base_tce_api_url.clone(),
                        positions: Vec::new(), // TODO: acquire from subnet
                    },
                )
                .await
                {
                    Ok((tce_proxy_worker, source_head_certificate)) => {
                        info!("TCE proxy client is restarted for the source subnet {:?} from the head {:?}",config.subnet_id, source_head_certificate);
                        let source_head_certificate_id =
                            source_head_certificate.map(|cert| cert.0.id);
                        (tce_proxy_worker, source_head_certificate_id)
                    }
                    Err(e) => {
                        panic!("Unable to create TCE Proxy: {e}");
                    }
                };

                self.tce_proxy_worker = tce_proxy_worker;
            }
        }
    }

    // Shutdown app
    #[allow(dead_code)]
    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.tce_proxy_worker.shutdown().await?;
        self.subnet_runtime_proxy_worker.shutdown().await?;

        Ok(())
    }
}
