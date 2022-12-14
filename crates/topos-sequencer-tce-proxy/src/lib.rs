//!
//! The module to handle incoming events from the friendly TCE node
//!

use crate::Error::InvalidChannelError;
use hyper::{body::Buf, header, Client, Method, Request};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::transport::channel;
use topos_core::{
    api::tce::v1::{
        api_service_client::ApiServiceClient,
        watch_certificates_request::{self},
        watch_certificates_response::{self},
        SubmitCertificateRequest, WatchCertificatesRequest, WatchCertificatesResponse,
    },
    uci::{Certificate, CertificateId, SubnetId},
};
use topos_sequencer_types::*;

const CERTIFICATE_OUTBOUND_CHANNEL_SIZE: usize = 100;
const CERTIFICATE_INBOUND_CHANNEL_SIZE: usize = 100;
const TCE_PROXY_COMMAND_CHANNEL_SIZE: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tonic transport error")]
    TonicTransportError {
        #[from]
        source: tonic::transport::Error,
    },
    #[error("tonic error")]
    TonicStatusError {
        #[from]
        source: tonic::Status,
    },
    #[error("invalid channel error")]
    InvalidChannelError,
    #[error("invalid tce endpoint error")]
    InvalidTceEndpoint,
    #[error("invalid subnet id error")]
    InvalidSubnetId,
}

pub struct TceProxyConfig {
    pub subnet_id: SubnetId,
    pub base_tce_api_url: String,
}

/// Proxy with the TCE
///
/// 1) Fetch the delivered certificates from the TCE
/// 2) Submit the new certificate to the TCE
pub struct TceProxyWorker {
    commands: mpsc::UnboundedSender<TceProxyCommand>,
    events: mpsc::UnboundedReceiver<TceProxyEvent>,
}

enum TceClientCommand {
    OpenStream {
        subnet_id: topos_core::uci::SubnetId,
    },
    SendCertificate {
        cert: Certificate,
    },
    Shutdown,
}

pub struct TceClient {
    subnet_id: topos_core::uci::SubnetId,
    tce_endpoint: String,
    command_sender: mpsc::Sender<TceClientCommand>,
}

impl TceClient {
    pub async fn open_stream(&self) -> Result<(), Error> {
        self.command_sender
            .send(TceClientCommand::OpenStream {
                subnet_id: self.subnet_id.clone(),
            })
            .await
            .map_err(|_| InvalidChannelError)?;
        Ok(())
    }
    pub async fn send_certificate(&mut self, cert: Certificate) -> Result<(), Error> {
        self.command_sender
            .send(TceClientCommand::SendCertificate { cert })
            .await
            .map_err(|_| InvalidChannelError)?;
        Ok(())
    }
    pub async fn close(&mut self) -> Result<(), Error> {
        self.command_sender
            .send(TceClientCommand::Shutdown)
            .await
            .map_err(|_| InvalidChannelError)?;
        Ok(())
    }
}

#[derive(Default)]
pub struct TceClientBuilder {
    tce_endpoint: Option<String>,
    subnet_id: Option<String>,
}

impl TceClientBuilder {
    pub fn set_tce_endpoint<T: ToString>(mut self, endpoint: T) -> Self {
        self.tce_endpoint = Some(endpoint.to_string());
        self
    }

    pub fn set_subnet_id<T: ToString>(mut self, subnet_id: T) -> Self {
        self.subnet_id = Some(subnet_id.to_string());
        self
    }

    pub async fn build_and_launch(
        self,
    ) -> Result<(TceClient, impl futures::stream::Stream<Item = Certificate>), Error> {
        // Channel used to pass received certificates (certificates pushed TCE node) from the TCE client to the application
        let (inbound_certificate_sender, inbound_certificate_receiver) =
            mpsc::channel(CERTIFICATE_INBOUND_CHANNEL_SIZE);

        let tce_endpoint = self
            .tce_endpoint
            .as_ref()
            .ok_or(Error::InvalidTceEndpoint)?
            .clone();
        // Connect to tce node service using backoff strategy
        let mut tce_grpc_client =
            match connect_to_tce_service_with_retry(tce_endpoint.clone()).await {
                Ok(client) => {
                    info!("Connected to tce service on endpoint {}", &tce_endpoint);
                    client
                }
                Err(e) => {
                    error!("Unable to connect to tce client, error details: {}", e);
                    return Err(e);
                }
            };

        // Channel used to initiate watch_certificates_request::Command that will be sent to the TCE through stream
        let (outbound_stream_command_sender, mut outbound_stream_command_receiver) =
            mpsc::channel::<WatchCertificatesRequest>(CERTIFICATE_OUTBOUND_CHANNEL_SIZE);

        // Outbound stream used to send watch_certificates_request::Command to the TCE node service
        let outbound_watch_certificates_stream = async_stream::stream! {
            loop {
                while let Some(request) = outbound_stream_command_receiver.recv().await {
                    yield request;
                }
            }
        };

        // Call TCE service watch certificates, get inbound response stream
        let mut inbound_watch_certificates_stream: tonic::Streaming<WatchCertificatesResponse> =
            tce_grpc_client
                .watch_certificates(outbound_watch_certificates_stream)
                .await
                .map(|r| r.into_inner())?;

        // Channel used to shut down task for inbound stream responses processing
        let (inbound_shutdown_sender, mut inbound_shutdown_receiver) =
            mpsc::unbounded_channel::<()>();

        let subnet_id = self
            .subnet_id
            .as_ref()
            .ok_or(Error::InvalidSubnetId)?
            .clone();
        //let tce_endpoint = self.tce_endpoint.ok_or(Error::InvalidTceEndpoint)?.clone();
        // Run task and process inbound watch certificate stream responses
        tokio::spawn(async move {
            // Listen for feedback from TCE service (WatchCertificatesResponse)
            info!(
                "Entering watch certificate response loop for tce node {} for subnet id {}",
                &tce_endpoint, &subnet_id
            );
            loop {
                tokio::select! {
                    Some(response) = inbound_watch_certificates_stream.next() => {
                        match response {
                            Ok(watch_certificate_response) => match watch_certificate_response.event {
                                // Received CertificatePushed event from TCE (new certificate has been received from TCE)
                                Some(watch_certificates_response::Event::CertificatePushed(
                                    certificate_pushed,
                                )) => {
                                    info!("Certificate {:?} received from tce", &certificate_pushed);
                                    if let Some(certificate) = certificate_pushed.certificate {
                                        if let Err(e) = inbound_certificate_sender
                                            .send(certificate.into())
                                            .await
                                        {
                                            error!(
                                                "unable to pass received certificate to application, error details: {}",
                                                e.to_string()
                                            )
                                        }
                                    }
                                }
                                // Confirmation from TCE that stream has been opened
                                Some(watch_certificates_response::Event::StreamOpened(stream_opened)) => {
                                    info!(
                                        "Watch certificate stream opened for for tce node {} subnet ids {:?}",
                                         &tce_endpoint, stream_opened.subnet_ids
                                    );
                                }
                                None => {
                                    warn!(
                                        "Watch certificate stream received None object from tce node {}", &tce_endpoint
                                    );
                                }
                            },
                            Err(e) => {
                                error!(
                                    "Watch certificates response error for tce node {} subnet_id {}, error details: {}",
                                    &tce_endpoint, &subnet_id, e.to_string()
                                )
                            }
                        }
                    }
                    Some(_) = inbound_shutdown_receiver.recv() => {
                        // Finish this task listener
                        break;
                    }
                }
            }
            info!(
                "Finishing watch certificate task for tce node {} subnet_id {}",
                &tce_endpoint, &subnet_id
            );
        });

        // Channel used to pass commands from the application to the TCE proxy
        // To close to chanel worker task, send None as Certificate
        let (tce_command_sender, mut tce_command_receiver) =
            mpsc::channel::<TceClientCommand>(TCE_PROXY_COMMAND_CHANNEL_SIZE);

        // Run task for sending certificates to the TCE stream
        let tce_endpoint = self
            .tce_endpoint
            .as_ref()
            .ok_or(Error::InvalidTceEndpoint)?
            .clone();
        tokio::spawn(async move {
            info!(
                "Entering tce proxy command loop for stream {}",
                &tce_endpoint
            );
            loop {
                tokio::select! {
                   command = tce_command_receiver.recv() => {
                        match command {
                           Some(TceClientCommand::SendCertificate {cert}) =>  {
                                let cert_id = cert.cert_id.clone();
                                let previous_cert_id = cert.prev_cert_id.clone();
                                match tce_grpc_client
                                .submit_certificate(SubmitCertificateRequest {
                                    certificate: Some(topos_core::api::uci::v1::Certificate::from(cert)),
                                })
                                .await
                                .map(|r| r.into_inner()) {
                                    Ok(_response)=> {
                                        info!("Certificate cert_id: {} previous_cert_id: {} successfully sent to tce {}", &cert_id, &previous_cert_id, &tce_endpoint);
                                    }
                                    Err(e) => {
                                        error!("Certificate submit failed, error details: {}", e);
                                    }
                                }
                            }
                            Some(TceClientCommand::OpenStream {subnet_id}) =>  {
                                // Send command to TCE to open stream with my subnet id
                                info!(
                                    "Sending OpenStream command to tce node {} for subnet id {}",
                                    &tce_endpoint, &subnet_id
                                );
                                if let Err(e) = outbound_stream_command_sender
                                    .send(
                                            watch_certificates_request::OpenStream {
                                                subnet_ids: vec![topos_core::api::shared::v1::SubnetId {
                                                    value: subnet_id.clone(),
                                                }],
                                            }.into(),
                                    )
                                    .await
                                    {
                                        error!(
                                            "Unable to send OpenStream command, error details: {}",
                                            e.to_string()
                                        )
                                    }
                            }
                            Some(TceClientCommand::Shutdown) =>  {
                                inbound_shutdown_sender.send(()).expect("valid channel for shutting down task");
                                break;
                            }
                            None => {
                                panic!("None should not be possible");
                            }
                        }
                    }
                }
            }
            info!(
                "Finished submit certificate loop for stream {}",
                &tce_endpoint
            );
            Result::<(), Error>::Ok(())
        });

        Ok((
            TceClient {
                subnet_id: self.subnet_id.ok_or(Error::InvalidSubnetId)?,
                tce_endpoint: self.tce_endpoint.ok_or(Error::InvalidTceEndpoint)?,
                command_sender: tce_command_sender,
            },
            tokio_stream::wrappers::ReceiverStream::new(inbound_certificate_receiver),
        ))
    }
}

/// Create new backoff library error based on error that happened
pub fn new_tce_proxy_backoff_err<E: std::fmt::Display>(err: E) -> backoff::Error<E> {
    // Retry according to backoff policy
    backoff::Error::Transient {
        err,
        retry_after: None,
    }
}

async fn connect_to_tce_service_with_retry(
    endpoint: String,
) -> Result<ApiServiceClient<tonic::transport::channel::Channel>, Error> {
    info!(
        "Connecting to tce service endpoint {} using backoff strategy...",
        endpoint
    );
    let op = || async {
        let channel = channel::Endpoint::from_shared(endpoint.clone())?
            .connect()
            .await
            .map_err(|e| {
                error!(
                    "Unable to connect to tce service {}, error details: {}",
                    &endpoint,
                    e.to_string()
                );
                e
            })?;
        Ok(ApiServiceClient::new(channel))
    };
    backoff::future::retry(backoff::ExponentialBackoff::default(), op)
        .await
        .map_err(|e| {
            error!("Error connecting to  service api {} ...", e.to_string());
            Error::TonicTransportError { source: e }
        })
}

impl TceProxyWorker {
    pub async fn new(config: TceProxyConfig) -> Result<Self, Error> {
        let (command_sender, mut command_rcv) = mpsc::unbounded_channel::<TceProxyCommand>();
        let (evt_sender, evt_rcv) = mpsc::unbounded_channel::<TceProxyEvent>();

        let (mut tce_client, mut receiving_certificate_stream) = TceClientBuilder::default()
            .set_subnet_id(&config.subnet_id)
            .set_tce_endpoint(&config.base_tce_api_url)
            .build_and_launch()
            .await?;
        tce_client.open_stream().await?;

        tokio::spawn(async move {
            info!(
                "Entering tce proxy worker loop to handle app commands for tce endpoint {}",
                &tce_client.tce_endpoint
            );
            loop {
                tokio::select! {
                    // process TCE proxy commands received from application
                    Some(cmd) = command_rcv.recv() => {
                        match cmd {
                            TceProxyCommand::SubmitCertificate(cert) => {
                                info!(
                                    "Submitting new certificate to the TCE network: {:?}",
                                    &cert
                                );
                                if let Err(e) = tce_client.send_certificate(cert).await {
                                    error!("failed to pass certificate to tce client, error details: {}", e);
                                }
                            }
                            TceProxyCommand::Exit => {
                                info!("Received TceProxyCommand::Exit command, closing tce client...");
                                if let Err(e) = tce_client.close().await {
                                    error!("Unable to shutdown tce client, error details: {}", e);
                                }
                                break;
                            }
                        }
                    }
                     // process certificates received from the TCE node
                    Some(cert) = receiving_certificate_stream.next() => {
                        info!("Received certificate from TCE {:?}", cert);
                        evt_sender.send(TceProxyEvent::NewDeliveredCerts(vec![cert])).expect("send");
                    }
                }
            }
            info!(
                "Exiting tce proxy worker handle loop for endpoint {}",
                &tce_client.tce_endpoint
            );
        });

        // Save channels and handles
        Ok(Self {
            commands: command_sender,
            events: evt_rcv,
        })
    }

    /// Send commands to TCE
    pub fn send_command(&self, cmd: TceProxyCommand) -> Result<(), String> {
        match self.commands.send(cmd) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Pollable (in select!) event listener
    pub async fn next_event(&mut self) -> Result<TceProxyEvent, String> {
        let event = self.events.recv().await;
        Ok(event.unwrap())
    }

    /// Calls for the new certificates
    pub async fn check_new_certificates(
        base_tce_api_url: &str,
        subnet_id: &SubnetId,
        from_cert_id: &CertificateId,
    ) -> Result<Vec<Certificate>, String> {
        let post_data = DeliveredCertsReq {
            subnet_id: subnet_id.clone(),
            from_cert_id: from_cert_id.clone(),
        };

        // setup the client
        let http_cli = Client::new();
        let delivered_certs_uri = (base_tce_api_url.to_string() + "/delivered_certs")
            .parse::<hyper::Uri>()
            .unwrap();

        let json = serde_json::to_string(&post_data).expect("serialize");

        let req = Request::builder()
            .method(Method::POST)
            .uri(delivered_certs_uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(json.into())
            .unwrap();

        match http_cli.request(req).await {
            Ok(http_res) => match hyper::body::aggregate(http_res).await {
                Ok(res_body) => {
                    match serde_json::from_reader::<_, DeliveredCerts>(res_body.reader()) {
                        Ok(new_certs) => {
                            log::debug!("check_new_certificates, post ok: {:?}", new_certs);
                            Ok(new_certs.certs)
                        }
                        Err(e) => {
                            log::warn!("check_new_certificates - failed to parse the body:{:?}", e);
                            Err(format!("{:?}", e))
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "check_new_certificates - failed to aggregate the body:{:?}",
                        e
                    );
                    Err(format!("{:?}", e))
                }
            },
            Err(e) => {
                log::warn!("check_new_certificates failed: {:?}", e);
                Err(format!("{:?}", e))
            }
        }
    }
}

#[derive(Serialize, Debug)]
pub struct SubmitCertReq {
    pub cert: Certificate,
}

#[derive(Serialize, Debug)]
struct DeliveredCertsReq {
    pub subnet_id: SubnetId,
    pub from_cert_id: CertificateId,
}

#[derive(Deserialize, Debug)]
struct DeliveredCerts {
    certs: Vec<Certificate>,
}

#[cfg(test)]
mod should {
    use crate::TceProxyWorker;

    #[tokio::test]
    async fn call_into_tce() {
        pretty_env_logger::init_timed();

        let jh = tokio::spawn(async move {
            let _ = TceProxyWorker::check_new_certificates(
                &"http://localhost:8080".to_string(),
                &100.to_string(),
                &0.to_string(),
            )
            .await;
        });
        let _ = tokio::join!(jh);
        log::debug!("ok");
    }
}
