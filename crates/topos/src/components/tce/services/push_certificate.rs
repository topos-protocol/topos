use futures::{future::join_all, StreamExt};
use rand::seq::SliceRandom;
use serde::Deserialize;
use std::time::Duration;
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};
use tokio::time::error::Elapsed;
use tonic::transport::Uri;
use topos_core::{
    api::grpc::{
        shared::v1::checkpoints::TargetCheckpoint,
        tce::v1::{
            api_service_client::ApiServiceClient,
            console_service_client::ConsoleServiceClient,
            watch_certificates_request::OpenStream,
            watch_certificates_response::{CertificatePushed, Event},
            StatusRequest, SubmitCertificateRequest,
        },
    },
    uci::{Certificate, CERTIFICATE_ID_LENGTH, SUBNET_ID_LENGTH},
};
use tracing::{debug, warn};

use crate::options::input_format::{InputFormat, Parser};

/// Picks a random peer and sends it a certificate. All other peers listen for broadcast certs.
/// Three possible outcomes:
/// 1. No errors, returns Ok;
/// 2. There were errors, returns a list of all errors encountered;
/// 3. timeout"
pub(crate) async fn check_certificate_delivery(
    timeout_broadcast: u64,
    peers: Vec<Uri>,
    timeout: u64,
) -> Result<Result<(), Vec<String>>, Elapsed> {
    tokio::time::timeout(Duration::from_secs(timeout), async move {
        let random_peer: Uri = peers
            .choose(&mut rand::thread_rng())
            .ok_or_else(|| {
                vec![format!(
                    "Unable to select a random peer from the list: {peers:?}"
                )]
            })?
            .try_into()
            .map_err(|_| vec![format!("Unable to parse the peer address")])?;

        let pushed_certificate = Certificate::new_with_default_fields(
            [0u8; CERTIFICATE_ID_LENGTH],
            [1u8; SUBNET_ID_LENGTH].into(),
            &[[2u8; SUBNET_ID_LENGTH].into()],
        )
        .map_err(|_| vec![format!("Unable to create the certificate")])?;
        let certificate_id = pushed_certificate.id;

        let mut join_handlers = Vec::new();

        // check that all nodes delivered the certificate
        for peer in peers {
            join_handlers.push(tokio::spawn(async move {
                let peer_string = peer.clone();
                let mut client = ConsoleServiceClient::connect(peer_string.clone())
                    .await
                    .map_err(|_| (peer_string.clone(), "Unable to connect to the api console"))?;

                let result = client.status(StatusRequest {}).await.map_err(|_| {
                    (
                        peer_string.clone(),
                        "Unable to get the status from the api console",
                    )
                })?;

                let status = result.into_inner();
                if !status.has_active_sample {
                    return Err((peer_string, "failed to find active sample"));
                }

                let mut client = ApiServiceClient::connect(peer_string.clone())
                    .await
                    .map_err(|_| (peer_string.clone(), "Unable to connect to the TCE api"))?;

                let in_stream = async_stream::stream! {
                    yield OpenStream {
                        target_checkpoint: Some(TargetCheckpoint {
                            target_subnet_ids: vec![[2u8; SUBNET_ID_LENGTH].into()],
                            positions: vec![]
                        }),
                        source_checkpoint: None
                    }.into()
                };

                let response = client.watch_certificates(in_stream).await.map_err(|_| {
                    (
                        peer_string.clone(),
                        "Unable to execute the watch_certificates on TCE api",
                    )
                })?;
                let mut resp_stream = response.into_inner();
                async move {
                    while let Some(received) = resp_stream.next().await {
                        let received = received.unwrap();
                        if let Some(Event::CertificatePushed(CertificatePushed {
                            certificate: Some(certificate),
                            ..
                        })) = received.event
                        {
                            // unwrap is safe because we are sure that the certificate is present
                            if certificate_id == certificate.id.unwrap() {
                                debug!("Received the certificate on {}", peer_string);
                                return Ok(());
                            }
                        }
                    }

                    Err((peer_string.clone(), "didn't receive any certificate"))
                }
                .await
            }));
        }

        let mut client = ApiServiceClient::connect(random_peer.clone())
            .await
            .map_err(|_| vec![format!("Unable to connect to the TCE api on {random_peer}")])?;

        // submit a certificate to one node
        _ = client
            .submit_certificate(SubmitCertificateRequest {
                certificate: Some(pushed_certificate.into()),
            })
            .await
            .map_err(|_| {
                vec![format!(
                    "Unable to submit the certificate to the TCE api on {random_peer}"
                )]
            })?;

        tokio::time::sleep(Duration::from_secs(timeout_broadcast)).await;
        let mut errors = vec![];

        join_all(join_handlers)
            .await
            .iter()
            .for_each(|result| match result {
                Err(_) => {
                    errors.push("Unable to properly execute command".to_string());
                }
                Ok(Err((peer, error))) => {
                    errors.push(format!("{peer} {error}"));
                }
                _ => {}
            });

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    })
    .await
    .map_err(|error| {
        warn!("Timeout reached: {:?}", error);
        error
    })
}

pub(crate) async fn check_delivery(
    timeout_broadcast: u64,
    format: InputFormat,
    peers: Option<String>,
    timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    debug!("peers: {:?}", peers);

    let peers: Vec<Uri> = format
        .parse(NodeList(peers))
        .map_err(|e| format!("Unable to parse node list: {e}"))?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Unable to parse node list: {e}"))?;

    match check_certificate_delivery(timeout_broadcast, peers, timeout).await {
        Ok(Err(e)) => Err(format!("Error with certificate delivery: {e:?}").into()),
        Err(e) => Err(Box::new(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("Timeout elapsed: {e}"),
        ))),
        Ok(_) => Ok(()),
    }
}

pub(crate) struct NodeList(pub(crate) Option<String>);

#[derive(Deserialize)]
struct FileNodes {
    nodes: Vec<String>,
}

impl Parser<NodeList> for InputFormat {
    type Result = Result<Vec<String>, io::Error>;

    fn parse(&self, NodeList(input): NodeList) -> Self::Result {
        let mut input_string = String::new();
        _ = match input {
            Some(path) if Path::new(&path).is_file() => {
                File::open(path)?.read_to_string(&mut input_string)?
            }
            Some(string) => {
                input_string = string;
                0
            }
            None => io::stdin().read_to_string(&mut input_string)?,
        };

        match self {
            InputFormat::Json => Ok(serde_json::from_str::<FileNodes>(&input_string)?.nodes),
            InputFormat::Plain => Ok(input_string
                .trim()
                .split(&[',', '\n'])
                .map(|s| s.trim().to_string())
                .collect()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::check_certificate_delivery;
    use rstest::*;
    use std::time::Duration;
    use topos_core::api::grpc::tce::v1::StatusRequest;
    use topos_test_sdk::tce::create_network;
    use tracing::{debug, info};

    #[rstest]
    #[test_log::test(tokio::test)]
    #[timeout(Duration::from_secs(120))]
    async fn assert_push_certificate_delivery() -> Result<(), Box<dyn std::error::Error>> {
        let mut peers_context = create_network(5, vec![]).await;

        let mut status: Vec<bool> = Vec::new();

        for (_peer_id, client) in peers_context.iter_mut() {
            let response = client
                .console_grpc_client
                .status(StatusRequest {})
                .await
                .expect("Can't get status");

            status.push(response.into_inner().has_active_sample);
        }

        assert!(status.iter().all(|s| *s));

        let nodes = peers_context
            .iter()
            .map(|peer| peer.1.api_entrypoint.clone())
            .collect::<Vec<_>>();

        debug!("Nodes used in test: {:?}", nodes);

        let assertion = async move {
            let peers: Vec<tonic::transport::Uri> = nodes
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()
                .map_err(|e| format!("Unable to parse node list: {e}"))
                .expect("Valid node list");

            match check_certificate_delivery(5, peers, 30).await {
                Ok(Err(e)) => {
                    panic!("Error with certificate delivery: {e:?}");
                }
                Err(e) => {
                    panic!("Timeout elapsed: {e}");
                }
                Ok(_) => {
                    info!("Check certificate delivery passed!");
                }
            }
        };

        tokio::time::timeout(Duration::from_secs(120), assertion)
            .await
            .unwrap();

        Ok(())
    }
}
