//!
//! The module to handle incoming events from the friendly TCE node
//!
pub mod client;
pub mod worker;

use opentelemetry::Context;
use tonic::transport::channel;
use topos_core::api::checkpoints::TargetStreamPosition;
use topos_core::{
    api::tce::v1::api_service_client::ApiServiceClient,
    uci::{Certificate, SubnetId},
};
use tracing::{error, info};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Tonic transport error")]
    TonicTransportError {
        #[from]
        source: tonic::transport::Error,
    },
    #[error("Tonic error")]
    TonicStatusError {
        #[from]
        source: tonic::Status,
    },
    #[error("Invalid channel error")]
    InvalidChannelError,
    #[error("Invalid tce endpoint error")]
    InvalidTceEndpoint,
    #[error("Invalid subnet id error")]
    InvalidSubnetId,
    #[error("Invalid certificate error")]
    InvalidCertificate,
    #[error("Hex conversion error {source}")]
    HexConversionError {
        #[from]
        source: hex::FromHexError,
    },
    #[error("Unable to get source head certificate for subnet id {subnet_id}: {details}")]
    UnableToGetSourceHeadCertificate {
        subnet_id: SubnetId,
        details: String,
    },
    #[error("Certificate source head empty for subnet id {subnet_id}")]
    SourceHeadEmpty { subnet_id: SubnetId },
    #[error("Unable to get last pending certificates for subnet id {subnet_id}: {details}")]
    UnableToGetLastPendingCertificates {
        subnet_id: SubnetId,
        details: String,
    },
}

#[derive(Debug)]
pub enum TceProxyCommand {
    /// Submit a newly created certificate to the TCE
    SubmitCertificate {
        cert: Box<Certificate>,
        ctx: Context,
    },

    /// Shutdown command
    Shutdown(tokio::sync::oneshot::Sender<()>),
}

#[derive(Debug, Clone)]
pub enum TceProxyEvent {
    /// New delivered certificate (and its position) fetched from the TCE network
    NewDeliveredCerts {
        certificates: Vec<(Certificate, u64)>,
        ctx: Context,
    },
    /// Failed watching certificates channel
    /// Requires restart of sequencer tce proxy
    WatchCertificatesChannelFailed,
}

pub struct TceProxyConfig {
    pub subnet_id: SubnetId,
    pub base_tce_api_url: String,
    pub positions: Vec<TargetStreamPosition>,
}

async fn connect_to_tce_service_with_retry(
    endpoint: String,
) -> Result<ApiServiceClient<tonic::transport::channel::Channel>, Error> {
    info!(
        "Connecting to the TCE at {} using backoff strategy...",
        endpoint
    );
    let op = || async {
        let channel = channel::Endpoint::from_shared(endpoint.clone())?
            .connect()
            .await
            .map_err(|e| {
                error!("Failed to connect to the TCE at {}: {e}", &endpoint);
                e
            })?;
        Ok(ApiServiceClient::new(channel))
    };
    backoff::future::retry(backoff::ExponentialBackoff::default(), op)
        .await
        .map_err(|e| {
            error!("Failed to connect to the TCE: {e}");
            Error::TonicTransportError { source: e }
        })
}
