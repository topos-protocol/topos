//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.
//!
use aggregate::RuntimeProxy;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::mpsc;
use topos_sequencer_types::*;

pub type Peer = String;

pub mod aggregate;
pub mod keystore;

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
    #[error("keystore error: {source}")]
    KeystoreError {
        #[from]
        source: eth_keystore::KeystoreError,
    },
}

#[derive(Debug, Clone)]
pub struct RuntimeProxyConfig {
    pub subnet_id: SubnetId,
    pub endpoint: String,
    pub subnet_contract: String,
    pub keystore_file: String,
    pub keystore_password: String,
}

/// Thread safe client to the protocol aggregate
#[derive(Debug)]
pub struct RuntimeProxyWorker {
    runtime_proxy: Arc<Mutex<RuntimeProxy>>,
    commands: mpsc::UnboundedSender<RuntimeProxyCommand>,
    events: mpsc::UnboundedReceiver<RuntimeProxyEvent>,
}

impl RuntimeProxyWorker {
    /// Creates new instance of the aggregate and returns proxy to it.
    /// New client instances to the same aggregate can be cloned from the returned one.
    /// Aggregate is spawned as new task.
    pub fn new(config: RuntimeProxyConfig) -> Result<Self, Error> {
        let runtime_proxy = RuntimeProxy::spawn_new(config)?;
        let (events_sender, events_rcv) = mpsc::unbounded_channel::<RuntimeProxyEvent>();
        let commands;
        {
            let mut runtime_proxy = runtime_proxy.lock().expect("Unable to get proxy runtime");
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
    pub fn eval(&self, cmd: RuntimeProxyCommand) -> Result<(), Error> {
        let _ = self.commands.send(cmd);
        Ok(())
    }

    /// Pollable (in select!) events' listener
    pub async fn next_event(&mut self) -> Result<RuntimeProxyEvent, Error> {
        let event = self.events.recv().await;
        Ok(event.unwrap())
    }
}

impl Clone for RuntimeProxyWorker {
    fn clone(&self) -> Self {
        let mut runtime_proxy = self
            .runtime_proxy
            .lock()
            .expect("Unable to get proxy runtime");
        let ch_commands = runtime_proxy.commands_channel.clone();

        let (events_sender, events_rcv) = mpsc::unbounded_channel::<RuntimeProxyEvent>();
        runtime_proxy.events_subscribers.push(events_sender);

        Self {
            runtime_proxy: self.runtime_proxy.clone(),
            commands: ch_commands,
            events: events_rcv,
        }
    }
}

pub mod testing {
    use super::*;

    pub fn get_runtime(runtime_proxy_worker: &RuntimeProxyWorker) -> Arc<Mutex<RuntimeProxy>> {
        runtime_proxy_worker.runtime_proxy.clone()
    }
}
