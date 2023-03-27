use futures::{future::join_all, StreamExt};
use rand::seq::SliceRandom;
use std::time::Duration;
use tokio::time::error::Elapsed;
use tonic::transport::Uri;
use topos_core::{
    api::{
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
use tracing::{debug, info, trace};

use crate::{
    components::checker::parser::NodeList,
    options::input_format::{InputFormat, Parser},
};

pub(crate) async fn check_delivery(
    timeout_broadcast: u64,
    format: InputFormat,
    peers: Option<String>,
    timeout: u64,
) -> Result<Result<(), Vec<String>>, Elapsed> {
    tokio::time::timeout(Duration::from_secs(timeout), async move {
        let peers: Vec<Uri> = format
            .parse(NodeList(peers))
            .map_err(|_| vec![format!("Unable to parse node list")])?
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| vec![format!("Unable to parse node list")])?;

        let random_peer: Uri = peers
            .choose(&mut rand::thread_rng())
            .ok_or(vec![format!(
                "Unable to select a random peer from the list: {peers:?}"
            )])?
            .try_into()
            .map_err(|_| vec![format!("Unable to parse the peer address")])?;

        let pushed_certificate = Certificate::new(
            [0u8; CERTIFICATE_ID_LENGTH],
            [1u8; SUBNET_ID_LENGTH].into(),
            Default::default(),
            Default::default(),
            &[[2u8; SUBNET_ID_LENGTH].into()],
            0,
            Default::default(),
        )
        .map_err(|_| vec![format!("Unable to create the certificate")])?;

        let certificate_id = pushed_certificate.id;
        let mut join_handlers = Vec::new();

        // check that every nodes delivered the certificate
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
                    return Err((peer_string, "didn't succeed in the sample phase"));
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
        info!("Timeout reached: {:?}", error);
        error
    })
}
