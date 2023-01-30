//! Protocol implementation guts.
//!

use crate::Error;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use topos_core::uci::{Certificate, CertificateId, SubnetId};
use topos_sequencer_types::{BlockInfo, CertificationCommand, CertificationEvent, SubnetEvent};
use tracing::{debug, error, warn};

const AVERAGE_BLOCK_TIME: Duration = Duration::from_secs(2);

pub struct Certification {
    pub commands_channel: mpsc::UnboundedSender<CertificationCommand>,
    pub events_subscribers: Vec<mpsc::UnboundedSender<CertificationEvent>>,
    _tx_exit: mpsc::UnboundedSender<()>,

    pub history: HashMap<SubnetId, Vec<CertificateId>>,
    pub finalized_blocks: Vec<BlockInfo>,
    pub subnet_id: SubnetId,
    pub verifier: u32,
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
    ) -> Result<Arc<Mutex<Certification>>, crate::Error> {
        let (command_sender, mut command_rcv) = mpsc::unbounded_channel::<CertificationCommand>();
        let (_tx_exit, mut rx_exit) = mpsc::unbounded_channel::<()>();

        // Initialize the certificate id history
        let mut history = HashMap::new();

        // Set last known certificate for my source subnet
        if let Some(cert_id) = source_head_certificate_id {
            history.insert(subnet_id, vec![cert_id]);
        }

        let me = Arc::new(Mutex::from(Self {
            commands_channel: command_sender,
            events_subscribers: Vec::new(),
            // todo: implement sync mechanism for the last seen cert
            _tx_exit,
            history,
            finalized_blocks: Vec::<BlockInfo>::new(),
            subnet_id,
            verifier,
        }));
        // Certification info for passing for async tasks
        let me_cl = me.clone();
        let me_c2 = me.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(AVERAGE_BLOCK_TIME); // arbitrary time for 1 block
            loop {
                interval.tick().await;
                if let Ok(certs) = Self::generate_certificates(me_c2.clone()) {
                    for cert in certs {
                        Self::send_new_certificate(
                            me_c2.clone(),
                            CertificationEvent::NewCertificate(cert),
                        );
                    }
                }
            }
        });

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Poll commands channel
                    cmd = command_rcv.recv() => {
                        Self::on_command(me_cl.clone(), cmd);
                    },
                    Some(_) = rx_exit.recv() => {
                        break;
                    }
                }
            }
        });
        Ok(me)
    }

    fn send_new_certificate(certification: Arc<Mutex<Certification>>, evt: CertificationEvent) {
        let mut certification = certification.lock().unwrap();
        certification.send_out_events(evt);
    }

    fn on_command(certification: Arc<Mutex<Certification>>, mb_cmd: Option<CertificationCommand>) {
        let mut certification = certification.lock().unwrap();

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

    fn send_out_events(&mut self, evt: CertificationEvent) {
        for tx in &self.events_subscribers {
            // FIXME: When error is returned it means that receiving side of the channel is closed
            // Thus we better remove the sender from our subscribers
            let _ = tx.send(evt.clone());
        }
    }

    /// Generation of Certificate
    fn generate_certificates(
        certification: Arc<Mutex<Certification>>,
    ) -> Result<Vec<Certificate>, Error> {
        let mut certification = certification.lock().unwrap();
        let subnet_id = certification.subnet_id;
        let mut generated_certificates = Vec::new();

        // For every block, create one certificate
        // This will change after MVP
        for block_info in &certification.finalized_blocks {
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

            // Get the id of the previous Certificate
            let previous_cert_id: CertificateId = match certification.history.get(&subnet_id) {
                Some(certs) => match certs.last() {
                    Some(cert_id) => *cert_id,
                    None => {
                        panic!("genesis certificate must be available for subnet {subnet_id:?}");
                    }
                },
                None => {
                    error!("ill-formed subnet history for {:?}", subnet_id);
                    return Err(Error::IllFormedSubnetHistory);
                }
            };

            let certificate = Certificate::new(
                previous_cert_id,
                subnet_id,
                block_info.state_root,
                block_info.tx_root_hash,
                &target_subnets,
                certification.verifier,
            )
            .map_err(|e| Error::CertificateGenerationError(e.to_string()))?;
            generated_certificates.push(certificate);
        }

        // Update history, clear pending finalized blocks
        let subnet_history = certification
            .history
            .get_mut(&subnet_id)
            .ok_or(Error::IllFormedSubnetHistory)?;

        // Just to be on the safe side remove all generated certificates that may exist in history
        let generated_certificates: Vec<Certificate> = generated_certificates
            .into_iter()
            .filter(|new_cert| !subnet_history.contains(&new_cert.id))
            .collect();

        subnet_history.extend(generated_certificates.iter().map(|cert| cert.id));

        certification.finalized_blocks.clear();

        Ok(generated_certificates)
    }
}
