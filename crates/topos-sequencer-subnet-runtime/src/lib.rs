//! implementation of Topos Reliable Broadcast to be used in the Transmission Control Engine (TCE)
//!
//! Abstracted from actual transport implementation.
//! Abstracted from actual storage implementation.
//!
use proxy::SubnetRuntimeProxy;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot};
use topos_core::api::grpc::checkpoints::TargetStreamPosition;
use topos_core::uci::{CertificateId, SubnetId};

pub type Peer = String;

pub mod certification;
pub mod proxy;

use crate::proxy::{SubnetRuntimeProxyCommand, SubnetRuntimeProxyEvent};

const EVENT_SUBSCRIBER_CHANNEL_SIZE: usize = 64;

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

    #[error("Invalid previous certificate id")]
    InvalidPreviousCertificateId,

    #[error("Ill formed subnet history")]
    IllFormedSubnetHistory,

    #[error("Unable to create certificate {0}")]
    CertificateGenerationError(String),

    #[error("Certificate signing error: {0}")]
    CertificateSigningError(topos_core::uci::Error),

    #[error("Unable to set source head certificate: {0}")]
    SourceHeadCertChannelError(String),

    #[error("Unable to send command: {0}")]
    CommandEvalChannelError(String),

    #[error("Invalid endpoint: {0}")]
    InvalidEndpoint(String),
}

#[derive(Debug, Clone)]
pub struct SubnetRuntimeProxyConfig {
    pub subnet_id: SubnetId,
    pub http_endpoint: String,
    pub ws_endpoint: String,
    pub subnet_contract_address: String,
    pub source_head_certificate_id: Option<CertificateId>,
    pub verifier: u32,
    pub start_block: Option<u64>,
}

/// Thread safe client to the protocol aggregate
#[derive(Debug)]
pub struct SubnetRuntimeProxyWorker {
    runtime_proxy: Arc<Mutex<SubnetRuntimeProxy>>,
    commands: mpsc::Sender<SubnetRuntimeProxyCommand>,
    events: mpsc::Receiver<SubnetRuntimeProxyEvent>,
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
        let (events_sender, events_rcv) =
            mpsc::channel::<SubnetRuntimeProxyEvent>(EVENT_SUBSCRIBER_CHANNEL_SIZE);
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
    pub async fn eval(&self, cmd: SubnetRuntimeProxyCommand) -> Result<(), Error> {
        self.commands
            .send(cmd)
            .await
            .map_err(|e| Error::CommandEvalChannelError(e.to_string()))
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

    pub async fn get_subnet_id(
        http_endpoint: &str,
        contract_address: &str,
    ) -> Result<SubnetId, Error> {
        SubnetRuntimeProxy::get_subnet_id(http_endpoint, contract_address).await
    }

    pub async fn set_source_head_certificate_id(
        &self,
        source_head_certificate_id: Option<(CertificateId, u64)>,
    ) -> Result<(), Error> {
        let mut runtime_proxy = self.runtime_proxy.lock().await;
        runtime_proxy
            .set_source_head_certificate_id(source_head_certificate_id)
            .await
    }
}

/// From the user provided subnet node endpoint (could be ip:port, http://ip:port, https://ip:port)
/// derive http and ws endpoints that will be used to communicate with the subnet
pub fn derive_endpoints(endpoint: &str) -> Result<(String, String), Error> {
    let http_endpoint: String;
    let ws_endpoint: String;

    if endpoint.starts_with("https") {
        // Use https endpoint as it is
        // Derive wss endpoint
        http_endpoint = endpoint.to_string();
        ws_endpoint = http_endpoint.replace("https", "wss") + "/ws";
    } else if endpoint.starts_with("http") {
        // Use http endpoint as it is
        // Derive ws endpoint
        http_endpoint = endpoint.to_string();
        ws_endpoint = http_endpoint.replace("http", "ws") + "/ws";
    } else {
        http_endpoint = format!("http://{}", endpoint);
        ws_endpoint = format!("ws://{}/ws", endpoint);
    }
    Ok((http_endpoint, ws_endpoint))
}

pub mod testing {
    use super::*;

    pub fn get_runtime(
        runtime_proxy_worker: &SubnetRuntimeProxyWorker,
    ) -> Arc<Mutex<SubnetRuntimeProxy>> {
        runtime_proxy_worker.runtime_proxy.clone()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_derive_endpoints() {
        use super::derive_endpoints;
        let (http_endpoint, ws_endpoint) = derive_endpoints("10.10.10.13:321").unwrap();
        assert_eq!(
            (http_endpoint.as_str(), ws_endpoint.as_str()),
            ("http://10.10.10.13:321", "ws://10.10.10.13:321/ws")
        );
        let (http_endpoint, ws_endpoint) = derive_endpoints("http://www.example.com").unwrap();
        assert_eq!(
            (http_endpoint.as_str(), ws_endpoint.as_str()),
            ("http://www.example.com", "ws://www.example.com/ws")
        );
        let (http_endpoint, ws_endpoint) = derive_endpoints("https://www.example.com:123").unwrap();
        assert_eq!(
            (http_endpoint.as_str(), ws_endpoint.as_str()),
            (
                "https://www.example.com:123",
                "wss://www.example.com:123/ws"
            )
        );
    }
}
