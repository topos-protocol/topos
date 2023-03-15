//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.
//!
use std::sync::Arc;
use subnet_runtime_proxy::SubnetRuntimeProxy;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use topos_core::api::checkpoints::TargetStreamPosition;
use topos_sequencer_types::*;

pub type Peer = String;

pub mod subnet_runtime_proxy;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Peers error: {err}")]
    BadPeers { err: String },

    #[error("Command error: {err}")]
    BadCommand { err: String },

    #[error("Tokio join error: {source}")]
    TokioError { source: tokio::task::JoinError },

    #[error("Failed to acquire locked object")]
    UnlockError,

    #[error("Unexpected type of transaction")]
    InvalidTransactionType,

    #[error("subnet client error: {source}")]
    SubnetError {
        #[from]
        source: topos_sequencer_subnet_client::Error,
    },

    #[error("Unable to retrieve key error: {source}")]
    UnableToRetrieveKey {
        #[from]
        source: topos_crypto::Error,
    },

    #[error("Unable to execute shutdown on the subnet runtime proxy: {0}")]
    ShutdownCommunication(mpsc::error::SendError<oneshot::Sender<()>>),

    #[error("Shutdown channel receive error {0}")]
    ShutdownSignalReceiveError(tokio::sync::oneshot::error::RecvError),
}

#[derive(Debug, Clone)]
pub struct SubnetRuntimeProxyConfig {
    pub subnet_id: SubnetId,
    pub endpoint: String,
    pub subnet_contract_address: String,
}

/// Thread safe client to the protocol aggregate
#[derive(Debug)]
pub struct SubnetRuntimeProxyWorker {
    runtime_proxy: Arc<Mutex<SubnetRuntimeProxy>>,
    commands: mpsc::UnboundedSender<SubnetRuntimeProxyCommand>,
    events: mpsc::UnboundedReceiver<SubnetRuntimeProxyEvent>,
}

impl SubnetRuntimeProxyWorker {
    /// Creates new instance of the aggregate and returns proxy to it.
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    pub async fn new(
        config: SubnetRuntimeProxyConfig,
        signing_key: Vec<u8>,
    ) -> Result<Self, Error> {
        let runtime_proxy = SubnetRuntimeProxy::spawn_new(config, signing_key)?;
        let (events_sender, events_rcv) = mpsc::unbounded_channel::<SubnetRuntimeProxyEvent>();
        let commands;
        {
            let mut runtime_proxy = runtime_proxy.lock().await;
            commands = runtime_proxy.commands_channel.clone();
            runtime_proxy.events_subscribers.push(events_sender);
        }

        Ok(Self {
            runtime_proxy,
            commands,
            events: events_rcv,
        })
    }

    /// Schedule command for execution
    pub fn eval(&self, cmd: SubnetRuntimeProxyCommand) -> Result<(), Error> {
        let _ = self.commands.send(cmd);
        Ok(())
    }

    /// Pollable (in select!) events' listener
    pub async fn next_event(&mut self) -> Result<SubnetRuntimeProxyEvent, Error> {
        let event = self.events.recv().await;
        Ok(event.unwrap())
    }

    /// Shutdown subnet runtime proxy worker
    pub async fn shutdown(&mut self) -> Result<(), Error> {
        let runtime_proxy = self.runtime_proxy.lock().await;
        runtime_proxy.shutdown().await
    }

    pub async fn get_checkpoints(&self) -> Result<Vec<TargetStreamPosition>, Error> {
        let runtime_proxy = self.runtime_proxy.lock().await;
        runtime_proxy.get_checkpoints().await
    }

    pub async fn get_subnet_id(endpoint: &str, contract_address: &str) -> Result<SubnetId, Error> {
        SubnetRuntimeProxy::get_subnet_id(endpoint, contract_address).await
    }
}

pub mod testing {
    use super::*;

    pub fn get_runtime(
        runtime_proxy_worker: &SubnetRuntimeProxyWorker,
    ) -> Arc<Mutex<SubnetRuntimeProxy>> {
        runtime_proxy_worker.runtime_proxy.clone()
    }
}
