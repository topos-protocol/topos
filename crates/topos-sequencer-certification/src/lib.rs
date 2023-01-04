//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.
//!
use aggregate::Certification;
use std::array::TryFromSliceError;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use topos_core::uci::SubnetId;
use topos_sequencer_types::*;
pub type Peer = String;

pub mod aggregate;

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
    CertificateGenerationError(Box<dyn std::error::Error>),
}

/// Thread safe client to the protocol aggregate
#[derive(Debug)]
pub struct CertificationWorker {
    aggr: Arc<Mutex<Certification>>,
    commands: mpsc::UnboundedSender<CertificationCommand>,
    events: mpsc::UnboundedReceiver<CertificationEvent>,
}

impl CertificationWorker {
    /// Creates new instance of the aggregate and returns proxy to it.
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    pub fn new(subnet_id: SubnetId) -> Result<Self, Error> {
        let w_aggr = Certification::spawn_new(subnet_id)?;
        let mut b_aggr = w_aggr.lock().map_err(|_| Error::LockError)?;
        let commands = b_aggr.commands_channel.clone();

        let (events_sender, events_rcv) = mpsc::unbounded_channel::<CertificationEvent>();
        b_aggr.events_subscribers.push(events_sender);

        Ok(Self {
            aggr: w_aggr.clone(),
            commands,
            events: events_rcv,
        })
    }

    /// Schedule command for execution
    pub fn eval(&self, event: Event) -> Result<(), Errors> {
        match event {
            Event::RuntimeProxyEvent(runtime_proxy_event) => match runtime_proxy_event {
                RuntimeProxyEvent::BlockFinalized(block_info) => {
                    let cmd = CertificationCommand::AddFinalizedBlock(block_info);
                    let _ = self.commands.send(cmd);
                }
                RuntimeProxyEvent::NewEra(_) => (),
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
    pub async fn next_event(&mut self) -> Result<CertificationEvent, Errors> {
        let event = self.events.recv().await;
        Ok(event.unwrap())
    }
}

impl Default for CertificationWorker {
    fn default() -> Self {
        Self::new([0u8; 32]).expect("valid default certificatino worker")
    }
}

impl Clone for CertificationWorker {
    fn clone(&self) -> Self {
        let mut aggr = self.aggr.lock().unwrap();
        let ch_commands = aggr.commands_channel.clone();

        let (events_sender, events_rcv) = mpsc::unbounded_channel::<CertificationEvent>();
        aggr.events_subscribers.push(events_sender);

        Self {
            aggr: self.aggr.clone(),
            commands: ch_commands,
            events: events_rcv,
        }
    }
}

/// Protocol and technical errors
#[derive(Debug)]
pub enum Errors {
    BadPeers {},
    BadCommand {},
    TokioError {},
}

impl From<mpsc::error::SendError<CertificationCommand>> for Errors {
    fn from(_arg: mpsc::error::SendError<CertificationCommand>) -> Self {
        Errors::TokioError {}
    }
}
