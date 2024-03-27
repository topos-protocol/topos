//!
//! Application logic glue
//!
use crate::CertificateProducerConfiguration;
use opentelemetry::trace::FutureExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use topos_certificate_producer_subnet_runtime::proxy::{
    SubnetRuntimeProxyCommand, SubnetRuntimeProxyEvent,
};
use topos_certificate_producer_subnet_runtime::SubnetRuntimeProxyWorker;
use topos_tce_proxy::{worker::TceProxyWorker, TceProxyCommand, TceProxyEvent};
use tracing::{debug, error, info, info_span, warn, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Top-level transducer certificate producer app context & driver (alike)
///
/// Implements <...Host> traits for network and Api, listens for protocol events in events
/// (store is not active component).
///
/// In the end we shall come to design where this struct receives
/// config+data as input and runs app returning data as output
///
pub struct AppContext {
    pub config: CertificateProducerConfiguration,
    pub subnet_runtime_proxy_worker: SubnetRuntimeProxyWorker,
    pub tce_proxy_worker: TceProxyWorker,
}

pub enum AppContextStatus {
    Finished,
    Restarting,
}

impl AppContext {
    /// Factory
    pub fn new(
        config: CertificateProducerConfiguration,
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
    pub(crate) async fn run(
        &mut self,
        shutdown: (CancellationToken, mpsc::Sender<()>),
    ) -> AppContextStatus {
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
                    match tce_evt {
                        TceProxyEvent::TceServiceFailure | TceProxyEvent::WatchCertificatesChannelFailed => {
                            // Unrecoverable failure in interaction with the TCE. Certificate Producer needs to be restarted
                            error!(
                                "Unrecoverable failure in Certificate Producer <-> TCE interaction. Shutting down Certificate Producer."
                            );
                            if let Err(e) = self.shutdown().await {
                                warn!("Failed to shutdown: {e:?}");
                            }
                            info!("Shutdown finished, restarting Certificate Producer...");
                            return AppContextStatus::Restarting;
                        },
                        _ => self.on_tce_proxy_event(tce_evt).await,
                    }
                },

                // Shutdown signal
                _ = shutdown.0.cancelled() => {
                    info!("Shutting down Certificate Producer app context...");
                    if let Err(e) = self.shutdown().await {
                        error!("Error shutting down Certificate Producer app context: {e}");
                    }
                    // Drop the sender to notify the Certificate Producer termination
                    drop(shutdown.1);
                    return AppContextStatus::Finished;
                }
            }
        }
    }

    async fn on_subnet_runtime_proxy_event(&mut self, evt: SubnetRuntimeProxyEvent) {
        debug!("on_subnet_runtime_proxy_event : {:?}", &evt);
        match evt {
            SubnetRuntimeProxyEvent::NewCertificate {
                cert,
                block_number: _,
                ctx,
            } => {
                let span = info_span!("Certificate Producer app context");
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
        if let TceProxyEvent::NewDeliveredCerts { certificates, ctx } = evt {
            let span = info_span!("Certificate Producer app context");
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
    }

    // Shutdown app
    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.tce_proxy_worker.shutdown().await?;
        self.subnet_runtime_proxy_worker.shutdown().await?;

        Ok(())
    }
}
