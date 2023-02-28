//!
//! Application logic glue
//!
use crate::SequencerConfiguration;
use serde::{Deserialize, Serialize};
use topos_sequencer_certification::CertificationWorker;
use topos_sequencer_subnet_runtime_proxy::SubnetRuntimeProxyWorker;
use topos_sequencer_tce_proxy::TceProxyWorker;
use topos_sequencer_types::*;
use tracing::{debug, info, warn};

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
    pub certification_worker: CertificationWorker,
    pub runtime_proxy_worker: SubnetRuntimeProxyWorker,
    pub tce_proxy_worker: TceProxyWorker,
}

impl AppContext {
    /// Factory
    pub fn new(
        config: SequencerConfiguration,
        certification_worker: CertificationWorker,
        runtime_proxy_worker: SubnetRuntimeProxyWorker,
        tce_proxy_worker: TceProxyWorker,
    ) -> Self {
        Self {
            config,
            certification_worker,
            runtime_proxy_worker,
            tce_proxy_worker,
        }
    }

    /// Main processing loop
    pub(crate) async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            tokio::select! {

                // Runtime View Worker
                Ok(evt) = self.runtime_proxy_worker.next_event() => {
                    debug!("runtime_proxy_worker.next_event(): {:?}", &evt);
                    self.on_runtime_proxy_event(evt).await;
                },

                // Certification Worker
                Ok(evt) = self.certification_worker.next_event() => {
                    debug!("certification_worker.next_event(): {:?}", &evt);
                    self.on_certification_event(evt).await;
                },

                // TCE events
                Ok(tce_evt) = self.tce_proxy_worker.next_event() => {
                    debug!("tce_proxy_worker.next_event(): {:?}", &tce_evt);
                    self.on_tce_proxy_event(tce_evt).await;
                },
            }
        }
    }

    async fn on_runtime_proxy_event(&mut self, evt: SubnetRuntimeProxyEvent) {
        debug!("on_runtime_proxy_event : {:?}", &evt);
        // This will always be a runtime proxy event
        let event = Event::RuntimeProxyEvent(evt);
        // TODO: error handling
        let _ = self.certification_worker.eval(event);
        //self.dkg_worker.eval(evt);
    }

    async fn on_certification_event(&mut self, evt: CertificationEvent) {
        debug!("on_certification_event : {:?}", &evt);
        match evt {
            CertificationEvent::NewCertificate(cert) => {
                self.tce_proxy_worker
                    .send_command(TceProxyCommand::SubmitCertificate(Box::new(cert)))
                    .expect("Submit Certificate to TCE");
            }
        }
    }

    async fn on_tce_proxy_event(&mut self, evt: TceProxyEvent) {
        match evt {
            TceProxyEvent::NewDeliveredCerts(certs) => {
                // New certificates acquired from tce, pass them to substrate runtime proxy
                // for processing
                for cert in certs {
                    self.runtime_proxy_worker
                        .eval(SubnetRuntimeProxyCommand::OnNewDeliveredTxns(cert))
                        .expect("Send cross transactions to the runtime");
                }
            }
            TceProxyEvent::WatchCertificatesChannelFailed => {
                warn!("Restarting tce proxy worker...");
                let config = &self.tce_proxy_worker.config;
                // Here try to restart tce proxy
                _ = self.tce_proxy_worker.shutdown().await;

                // TODO: Retrieve subnet checkpoint from where to start receiving certificates, again
                let (tce_proxy_worker, _source_head_certificate_id) = match TceProxyWorker::new(
                    topos_sequencer_tce_proxy::TceProxyConfig {
                        subnet_id: config.subnet_id,
                        base_tce_api_url: config.base_tce_api_url.clone(),
                        positions: Vec::new(), // TODO acquire from subnet
                    },
                )
                .await
                {
                    Ok((tce_proxy_worker, source_head_certificate)) => {
                        info!("TCE proxy client is restarted for the source subnet {:?} from the head {:?}",config.subnet_id, source_head_certificate);
                        let source_head_certificate_id =
                            source_head_certificate.map(|cert| cert.id);
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
        self.certification_worker.shutdown().await?;
        self.runtime_proxy_worker.shutdown().await?;

        Ok(())
    }
}

/// Definition of networking payload.
///
/// We assume that only Commands will go through the network,
/// [Response] is used to allow reporting of logic errors to the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum NetworkMessage {
    Cmd(TceCommands),
    Response(Result<(), String>),
}

// deserializer
impl From<Vec<u8>> for NetworkMessage {
    fn from(data: Vec<u8>) -> Self {
        bincode::deserialize::<NetworkMessage>(data.as_ref()).expect("msg deser")
    }
}

// serializer
impl From<NetworkMessage> for Vec<u8> {
    fn from(msg: NetworkMessage) -> Self {
        bincode::serialize::<NetworkMessage>(&msg).expect("msg ser")
    }
}

// transformer of protocol commands into network commands
impl From<TceCommands> for NetworkMessage {
    fn from(cmd: TceCommands) -> Self {
        Self::Cmd(cmd)
    }
}
