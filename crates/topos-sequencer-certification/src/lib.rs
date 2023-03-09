//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.
//!
use certification::Certification;
use std::array::TryFromSliceError;
use std::sync::Arc;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use topos_core::uci::{CertificateId, SubnetId};
use topos_sequencer_types::*;
use tracing::{error, info};
pub type Peer = String;

pub mod certification;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to lock object")]
    LockError,
    #[error("Invalid address conversion: {0}")]
    InvalidAddress(TryFromSliceError),
    #[error("Certificate empty")]
    EmptyCertificate,
    #[error("Ill formed subnet history")]
    IllFormedSubnetHistory,
    #[error("Unable to create certificate {0}")]
    CertificateGenerationError(String),
    #[error("Command channel error {0}")]
    CommandSendError(String),
    #[error("Unable to execute shutdown: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),
    #[error("Shutdown channel receive error {0}")]
    ShutdownSignalReceiveError(RecvError),
    #[error("Unable to retrieve key error: {0}")]
    UnableToRetrieveKey(#[from] topos_crypto::Error),
    #[error("Certificate signing error: {0}")]
    CertificateSigningError(topos_core::uci::Error),
}

/// Thread safe client to the protocol aggregate
#[derive(Debug)]
pub struct CertificationWorker {
    certification: Arc<Mutex<Certification>>,
    commands: mpsc::UnboundedSender<CertificationCommand>,
    events: mpsc::UnboundedReceiver<CertificationEvent>,
}

impl CertificationWorker {
    /// Creates new instance of the aggregate and returns proxy to it.
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    pub async fn new(
        subnet_id: SubnetId,
        source_head_certificate_id: Option<CertificateId>,
        verifier: u32,
        signing_key: Vec<u8>,
    ) -> Result<Self, Error> {
        let w_aggr =
            Certification::spawn_new(subnet_id, source_head_certificate_id, verifier, signing_key)?;
        let mut b_aggr = w_aggr.lock().await;
        let commands = b_aggr.commands_channel.clone();

        let (events_sender, events_rcv) = mpsc::unbounded_channel::<CertificationEvent>();
        b_aggr.events_subscribers.push(events_sender);

        Ok(Self {
            certification: w_aggr.clone(),
            commands,
            events: events_rcv,
        })
    }

    /// Schedule command for execution
    pub fn eval(&self, event: Event) -> Result<(), Error> {
        match event {
            Event::RuntimeProxyEvent(runtime_proxy_event) => match runtime_proxy_event {
                SubnetRuntimeProxyEvent::BlockFinalized(block_info) => {
                    let cmd = CertificationCommand::AddFinalizedBlock(block_info);
                    let _ = self.commands.send(cmd);
                }
                SubnetRuntimeProxyEvent::NewEra(_) => (),
            },
            Event::CertificationEvent(certification_event) => match certification_event {
                CertificationEvent::NewCertificate(_cert) => {
                    unimplemented!();
                }
            },
        }
        Ok(())
    }

    /// Pollable (in select!) events' listener
    pub async fn next_event(&mut self) -> Result<CertificationEvent, Error> {
        let event = self.events.recv().await;
        Ok(event.unwrap())
    }

    /// Shut down certificate worker
    pub async fn shutdown(&mut self) -> Result<(), Error> {
        info!("Shutting down certification worker...");
        let mut certification = self.certification.lock().await;
        certification.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    use topos_test_sdk::sequencer::TEST_VALIDATOR_KEY;

    const TEST_SUBNET_ID: SubnetId = topos_test_sdk::constants::SOURCE_SUBNET_ID_1;
    const TEST_CERTIFICATE_ID: CertificateId = topos_test_sdk::constants::CERTIFICATE_ID_1;

    #[test(tokio::test)]
    async fn instantiate_certification_worker() {
        let private_key = hex::decode(TEST_VALIDATOR_KEY).unwrap();
        // Launch the certification worker for certificate production
        let _cert_worker = match CertificationWorker::new(
            TEST_SUBNET_ID,
            Some(TEST_CERTIFICATE_ID),
            0,
            private_key,
        )
        .await
        {
            Ok(cert_worker) => cert_worker,
            Err(e) => {
                panic!("Unable to create certification worker: {e:?}");
            }
        };
    }

    #[test(tokio::test)]
    async fn certification_worker_eval() {
        let private_key = hex::decode(TEST_VALIDATOR_KEY).unwrap();
        // Launch the certification worker for certificate production
        let cert_worker = match CertificationWorker::new(
            TEST_SUBNET_ID,
            Some(TEST_CERTIFICATE_ID),
            0,
            private_key,
        )
        .await
        {
            Ok(cert_worker) => cert_worker,
            Err(e) => {
                panic!("Unable to create certification worker: {e:?}")
            }
        };

        let event = Event::RuntimeProxyEvent(SubnetRuntimeProxyEvent::BlockFinalized(BlockInfo {
            number: BlockNumber::from(10 as u64),
            ..Default::default()
        }));
        match cert_worker.eval(event) {
            Ok(_) => {}
            Err(e) => {
                panic!("Unable to evaluate certification event: {e:?}")
            }
        }
    }
}
