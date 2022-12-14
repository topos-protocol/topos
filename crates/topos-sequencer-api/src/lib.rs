//! External (non-Topos network) APIs of the node.
//! Implementation is divided into transport agnostic part
//! and protocol adapters - json-http, etc
mod web_api;

use tokio::sync::mpsc;

/// API configuration struct.
pub struct ApiConfig {
    // todo endpoints config
    pub web_port: u16,
}

/// Host (app context) interface
///
/// General pattern to support 'synchronous' call is to pass oneshot Sender within every request
/// and await on the Receiver side of the channel.
#[derive(Debug)]
pub enum ApiRequests {}

/// Umbrella worker for all api services
pub struct ApiWorker {
    pub rx_requests: mpsc::Receiver<ApiRequests>,
}

impl ApiWorker {
    pub fn new(config: ApiConfig) -> Self {
        let (tx, rx) = mpsc::channel(255);
        let me = Self { rx_requests: rx };
        let web_port = config.web_port;
        tokio::spawn(async move {
            web_api::run(tx.clone(), web_port).await;
        });

        me
    }

    /// 'Selectable' poll
    pub async fn next_request(&mut self) -> Result<ApiRequests, ()> {
        match self.rx_requests.recv().await {
            Some(e) => Ok(e),
            _ => Err(()),
        }
    }
}
