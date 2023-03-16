use crate::Error;
use opentelemetry::trace::FutureExt;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::{self, Duration};
use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_sequencer_types::{BlockInfo, CertificationCommand, CertificationEvent, SubnetEvent};
use tracing::{debug, error, field, info, instrument, warn, Instrument, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

const AVERAGE_BLOCK_TIME: Duration = Duration::from_secs(2);

pub struct Certification {
    pub commands_channel: mpsc::Sender<CertificationCommand>,
    pub events_subscribers: Vec<mpsc::Sender<CertificationEvent>>,
    pub history: HashMap<SubnetId, Vec<CertificateId>>,
    pub finalized_blocks: Vec<BlockInfo>,
    pub subnet_id: SubnetId,
    pub verifier: u32,
    command_shutdown: mpsc::Sender<oneshot::Sender<()>>,
    cert_gen_shutdown: mpsc::Sender<oneshot::Sender<()>>,
    signing_key: Vec<u8>,
}

impl Debug for Certification {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Certification instance").finish()
    }
}

impl Certification {
    pub fn spawn_new(
        subnet_id: SubnetId,
        source_head_certificate_id: Option<CertificateId>,
        verifier: u32,
        signing_key: Vec<u8>,
    ) -> Result<Arc<Mutex<Certification>>, crate::Error> {
        let (command_sender, mut command_rcv) = mpsc::channel::<CertificationCommand>(128);
        let (command_shutdown_channel, mut command_shutdown) =
            mpsc::channel::<oneshot::Sender<()>>(1);
        let (cert_gen_shutdown_channel, mut cert_gen_shutdown) =
            mpsc::channel::<oneshot::Sender<()>>(1);

        // Initialize the certificate id history
        let mut history = HashMap::new();

        // Set last known certificate for my source subnet
        if let Some(cert_id) = source_head_certificate_id {
            history.insert(subnet_id, vec![cert_id]);
        }

        let me = Arc::new(Mutex::from(Self {
            commands_channel: command_sender,
            events_subscribers: Vec::new(),
            history,
            finalized_blocks: Vec::<BlockInfo>::new(),
            subnet_id,
            verifier,
            command_shutdown: command_shutdown_channel,
            cert_gen_shutdown: cert_gen_shutdown_channel,
            signing_key,
        }));
        // Certification info for passing for async tasks
        let me_cl = me.clone();
        let me_c2 = me.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(AVERAGE_BLOCK_TIME); // arbitrary time for 1 block
            let shutdowned: Option<oneshot::Sender<()>> = loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Ok(certs) = Self::generate_certificates(me_c2.clone()).await {
                            for cert in certs {
                                Self::send_new_certificate(
                                    me_c2.clone(),
                                    cert
                                )
                                .await;
                            }
                        }
                    },
                    shutdown = cert_gen_shutdown.recv() => {
                        break shutdown;
                    }
                }
            };

            if let Some(sender) = shutdowned {
                info!("Shutting down generation of certificates");
                _ = sender.send(());
            }
        });

        tokio::spawn(async move {
            let shutdowned: Option<oneshot::Sender<()>> = loop {
                tokio::select! {
                    // Poll commands channel
                    cmd = command_rcv.recv() => {
                        Self::on_command(me_cl.clone(), cmd).await;
                    },
                    shutdown = command_shutdown.recv() => {
                        break shutdown;
                    }
                }
            };

            if let Some(sender) = shutdowned {
                info!("Shutting down certificate command processing");
                _ = sender.send(());
            }
        });
        Ok(me)
    }

    #[instrument(name = "NewCertificate", fields(certification = field::Empty, source_subnet_id = field::Empty, certificate_id = field::Empty))]
    async fn send_new_certificate(certification: Arc<Mutex<Certification>>, cert: Certificate) {
        let mut certification = certification.lock().await;
        Span::current().record("certificate_id", cert.id.to_string());
        Span::current().record("source_subnet_id", cert.source_subnet_id.to_string());

        certification
            .send_out_events(CertificationEvent::NewCertificate {
                cert,
                ctx: Span::current().context(),
            })
            .with_current_context()
            .instrument(Span::current())
            .await;
    }

    async fn on_command(
        certification: Arc<Mutex<Certification>>,
        mb_cmd: Option<CertificationCommand>,
    ) {
        let mut certification = certification.lock().await;

        match mb_cmd {
            Some(cmd) => match cmd {
                CertificationCommand::AddFinalizedBlock(block_info) => {
                    certification.finalized_blocks.push(block_info);
                    debug!(
                        "Finalized blocks mempool updated: {:?}",
                        &certification.finalized_blocks
                    );
                }
            },
            _ => {
                warn!("Empty command was passed");
            }
        }
    }

    async fn send_out_events(&mut self, evt: CertificationEvent) {
        self.events_subscribers.retain(|tx| {
            if let Ok(reserve) = tx.try_reserve() {
                reserve.send(evt.clone());
                true
            } else {
                error!("Unable to send certification event to event subscribers");
                false
            }
        });
    }

    /// Generation of Certificate
    async fn generate_certificates(
        certification: Arc<Mutex<Certification>>,
    ) -> Result<Vec<Certificate>, Error> {
        let mut certification = certification.lock().await;
        let subnet_id = certification.subnet_id;
        let mut generated_certificates = Vec::new();

        // For every block, create one certificate
        // This will change after MVP
        for block_info in &certification.finalized_blocks {
            // Parse target subnets from events
            let mut target_subnets: Vec<SubnetId> = Vec::new();
            for event in &block_info.events {
                match event {
                    SubnetEvent::TokenSent {
                        target_subnet_id, ..
                    } => {
                        target_subnets.push(*target_subnet_id);
                    }
                    SubnetEvent::ContractCall {
                        target_subnet_id, ..
                    } => {
                        target_subnets.push(*target_subnet_id);
                    }
                    SubnetEvent::ContractCallWithToken {
                        target_subnet_id, ..
                    } => {
                        target_subnets.push(*target_subnet_id);
                    }
                }
            }

            // Get the id of the previous Certificate from local history
            let previous_cert_id: CertificateId = match certification.history.get(&subnet_id) {
                Some(certs) => match certs.last() {
                    Some(cert_id) => *cert_id,
                    None => {
                        panic!("genesis certificate must be available for subnet {subnet_id:?}");
                    }
                },
                None => {
                    error!("Ill-formed subnet history for {:?}", subnet_id);
                    return Err(Error::IllFormedSubnetHistory);
                }
            };

            // TODO: acquire proof
            let proof = Vec::new();

            let mut certificate = Certificate::new(
                previous_cert_id,
                subnet_id,
                block_info.state_root,
                block_info.tx_root_hash,
                &target_subnets,
                certification.verifier,
                proof,
            )
            .map_err(|e| Error::CertificateGenerationError(e.to_string()))?;
            certificate
                .update_signature(certification.get_signing_key())
                .map_err(Error::CertificateSigningError)?;
            generated_certificates.push(certificate);
        }

        // Update history, clear pending finalized blocks
        let subnet_history = certification
            .history
            .get_mut(&subnet_id)
            .ok_or(Error::IllFormedSubnetHistory)?;

        for new_cert in &generated_certificates {
            if subnet_history.contains(&new_cert.id) {
                // This should not happen
                panic!("Same certificate generated multiple times: {new_cert:?}");
            }
        }

        subnet_history.extend(generated_certificates.iter().map(|cert| cert.id));

        certification.finalized_blocks.clear();

        Ok(generated_certificates)
    }

    pub fn get_signing_key(&self) -> &[u8] {
        self.signing_key.as_slice()
    }

    // Shutdown certification entity
    pub async fn shutdown(&mut self) -> Result<(), Error> {
        let (command_sender, command_receiver) = oneshot::channel();
        self.command_shutdown
            .send(command_sender)
            .await
            .map_err(Error::ShutdownCommunication)?;
        command_receiver
            .await
            .map_err(Error::ShutdownSignalReceiveError)?;

        let (cert_gen_sender, cert_gen_receiver) = oneshot::channel();
        self.cert_gen_shutdown
            .send(cert_gen_sender)
            .await
            .map_err(Error::ShutdownCommunication)?;
        cert_gen_receiver
            .await
            .map_err(Error::ShutdownSignalReceiveError)?;

        Ok(())
    }
}
