use base64::Engine;
use futures::StreamExt;
use rstest::*;
use std::collections::HashMap;
use test_log::test;
use tokio::time::Duration;
use topos_core::api::grpc::shared::v1::positions::SourceStreamPosition;
use topos_core::api::grpc::shared::v1::{
    checkpoints::TargetCheckpoint, positions::TargetStreamPosition,
};
use topos_core::api::grpc::shared::v1::{CertificateId, StarkProof, SubnetId};
use topos_core::api::grpc::tce::v1::LastPendingCertificate;
use topos_core::api::grpc::tce::v1::{
    watch_certificates_request, watch_certificates_response,
    watch_certificates_response::CertificatePushed, GetLastPendingCertificatesRequest,
    GetLastPendingCertificatesResponse, GetSourceHeadRequest, GetSourceHeadResponse,
    SubmitCertificateRequest,
};
use topos_core::api::grpc::uci::v1::Certificate;
use topos_core::types::CertificateDelivered;
use topos_core::uci::SUBNET_ID_LENGTH;
use topos_tce_proxy::worker::TceProxyWorker;
use topos_tce_proxy::{TceProxyCommand, TceProxyConfig};
use tracing::{debug, error, info, warn};

use topos_test_sdk::{
    certificates::create_certificate_chain,
    constants::*,
    tce::{start_node, TceContext},
};

pub const SOURCE_SUBNET_ID_1_NUMBER_OF_PREFILLED_CERTIFICATES: usize = 15;
pub const SOURCE_SUBNET_ID_2_NUMBER_OF_PREFILLED_CERTIFICATES: usize = 10;

#[rstest]
#[test(tokio::test)]
async fn test_tce_submit_certificate(
    #[future] start_node: TceContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = start_node.await;

    let source_subnet_id: SubnetId = SOURCE_SUBNET_ID_1.into();
    let prev_certificate_id: CertificateId = CERTIFICATE_ID_1.into();
    let certificate_id: CertificateId = CERTIFICATE_ID_2.into();

    match context
        .api_grpc_client
        .submit_certificate(SubmitCertificateRequest {
            certificate: Some(Certificate {
                source_subnet_id: Some(source_subnet_id.clone()),
                id: Some(certificate_id),
                prev_id: Some(prev_certificate_id),
                target_subnets: vec![],
                state_root: [0u8; 32].to_vec(),
                tx_root_hash: [0u8; 32].to_vec(),
                receipts_root_hash: [0u8; 32].to_vec(),
                verifier: 0,
                proof: Some(StarkProof { value: Vec::new() }),
                signature: Some(Default::default()),
            }),
        })
        .await
        .map(|r| r.into_inner())
    {
        Ok(response) => {
            debug!("Certificate successfully submitted {:?}", response);
        }
        Err(e) => {
            error!("Unable to submit the certificate: {e:?}");
            return Err(Box::from(e));
        }
    };
    info!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}

#[rstest]
#[test(tokio::test)]
async fn test_tce_watch_certificates(
    #[future] start_node: TceContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = start_node.await;

    let source_subnet_id: SubnetId = SubnetId {
        value: [1u8; SUBNET_ID_LENGTH].to_vec(),
    };

    //Outbound stream
    let subnet_id_instream = source_subnet_id.clone();
    let in_stream = async_stream::stream! {
        yield watch_certificates_request::OpenStream {
            target_checkpoint: Some(TargetCheckpoint {
                target_subnet_ids: vec![ subnet_id_instream ],
                positions: Vec::new()
            }),
            source_checkpoint: None
        }.into()
    };

    let response = context
        .api_grpc_client
        .watch_certificates(in_stream)
        .await
        .unwrap();

    let mut resp_stream = response.into_inner();

    info!("TCE client: waiting for watch certificate response");
    while let Some(received) = resp_stream.next().await {
        info!("TCE client received: {:?}", received);
        let received = received.unwrap();
        match received.event {
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: Some(certificate),
                ..
            })) => {
                info!("Certificate received {:?}", certificate);
            }
            Some(watch_certificates_response::Event::StreamOpened(
                watch_certificates_response::StreamOpened { subnet_ids },
            )) => {
                debug!("TCE client: stream opened for subnet_ids {:?}", subnet_ids);
                assert_eq!(subnet_ids[0].value, source_subnet_id.value);
                // We have opened connection and 2 way stream, finishing test
                break;
            }
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: None,
                ..
            })) => {
                panic!("TCE client: empty certificate received");
            }
            _ => {
                panic!("TCE client: something unexpected is received");
            }
        }
    }
    info!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}

#[rstest]
#[test(tokio::test)]
async fn test_tce_get_source_head_certificate(
    #[future] start_node: TceContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = start_node.await;

    let source_subnet_id: SubnetId = SOURCE_SUBNET_ID_1.into();
    let default_cert_id: CertificateId = PREV_CERTIFICATE_ID.into();
    let certificate_id: CertificateId = CERTIFICATE_ID_2.into();

    // Test get source head certificate for empty TCE history
    // This will be actual genesis certificate
    let response = context
        .api_grpc_client
        .get_source_head(GetSourceHeadRequest {
            subnet_id: Some(source_subnet_id.clone()),
        })
        .await
        .map(|r| r.into_inner())
        .expect("valid response");

    let expected_default_genesis_certificate = Certificate {
        id: Some(default_cert_id.clone()),
        prev_id: Some(default_cert_id.clone()),
        source_subnet_id: Some(source_subnet_id.clone()),
        target_subnets: vec![],
        state_root: [0u8; 32].to_vec(),
        tx_root_hash: [0u8; 32].to_vec(),
        receipts_root_hash: [0u8; 32].to_vec(),
        verifier: 0,
        proof: Some(StarkProof { value: Vec::new() }),
        signature: Some(Default::default()),
    };
    let expected_response = GetSourceHeadResponse {
        certificate: Some(expected_default_genesis_certificate.clone()),
        position: Some(SourceStreamPosition {
            source_subnet_id: Some(source_subnet_id.clone()),
            certificate_id: expected_default_genesis_certificate.id.clone(),
            position: 0,
        }),
    };

    assert_eq!(response, expected_response);

    let test_certificate = Certificate {
        source_subnet_id: Some(source_subnet_id.clone()),
        id: Some(certificate_id),
        prev_id: Some(default_cert_id),
        target_subnets: vec![],
        state_root: [0u8; 32].to_vec(),
        tx_root_hash: [0u8; 32].to_vec(),
        receipts_root_hash: [0u8; 32].to_vec(),
        verifier: 0,
        proof: Some(StarkProof { value: Vec::new() }),
        signature: Some(Default::default()),
    };

    match context
        .api_grpc_client
        .submit_certificate(SubmitCertificateRequest {
            certificate: Some(test_certificate.clone()),
        })
        .await
        .map(|r| r.into_inner())
    {
        Ok(response) => {
            debug!("Successfully submitted the Certificate {:?}", response);
        }
        Err(e) => {
            error!("Unable to submit the certificate: {e:?}");
            return Err(Box::from(e));
        }
    };

    // Test get source head certificate for non empty certificate history
    let response = context
        .api_grpc_client
        .get_source_head(GetSourceHeadRequest {
            subnet_id: Some(source_subnet_id.clone()),
        })
        .await
        .map(|r| r.into_inner())
        .unwrap();

    // TODO: currently only delivered certificates are counted as
    // head source certificate, so default certificate is expected
    // Should be updated to count also pending certificates
    let expected_response = GetSourceHeadResponse {
        certificate: Some(expected_default_genesis_certificate.clone()),
        position: Some(SourceStreamPosition {
            source_subnet_id: Some(source_subnet_id.clone()),
            certificate_id: expected_default_genesis_certificate.id,
            position: 0,
        }),
    };
    assert_eq!(response, expected_response);

    info!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}

#[rstest]
#[test(tokio::test)]
async fn test_tce_get_last_pending_certificates(
    #[future] start_node: TceContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = start_node.await;

    let source_subnet_id: SubnetId = SOURCE_SUBNET_ID_1.into();
    let certificates = create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], 10);

    // Test get last pending certificates for empty TCE history
    // Reply should be empty
    let response = context
        .api_grpc_client
        .get_last_pending_certificates(GetLastPendingCertificatesRequest {
            subnet_ids: vec![source_subnet_id.clone()],
        })
        .await
        .map(|r| r.into_inner())
        .expect("valid response");

    let last_pending_certificates = vec![(
        base64::engine::general_purpose::STANDARD.encode(&source_subnet_id.value),
        LastPendingCertificate {
            value: None,
            index: 0,
        },
    )]
    .into_iter()
    .collect::<HashMap<String, LastPendingCertificate>>();

    let expected_response = GetLastPendingCertificatesResponse {
        last_pending_certificate: last_pending_certificates,
    };

    assert_eq!(response, expected_response);

    for cert in &certificates {
        match context
            .api_grpc_client
            .submit_certificate(SubmitCertificateRequest {
                certificate: Some(cert.certificate.clone().into()),
            })
            .await
            .map(|r| r.into_inner())
        {
            Ok(response) => {
                debug!("Successfully submitted the Certificate {:?}", response);
            }
            Err(e) => {
                error!("Unable to submit the certificate: {e:?}");
                return Err(Box::from(e));
            }
        };
    }

    // Test get last pending certificate
    let response = context
        .api_grpc_client
        .get_last_pending_certificates(GetLastPendingCertificatesRequest {
            subnet_ids: vec![source_subnet_id.clone()],
        })
        .await
        .map(|r| r.into_inner())
        .expect("valid response");

    let expected_last_pending_certificates = vec![(
        base64::engine::general_purpose::STANDARD.encode(&source_subnet_id.value),
        LastPendingCertificate {
            value: Some(
                certificates
                    .iter()
                    .last()
                    .unwrap()
                    .clone()
                    .certificate
                    .into(),
            ),
            index: 10,
        },
    )]
    .into_iter()
    .collect::<HashMap<String, LastPendingCertificate>>();

    let expected_response = GetLastPendingCertificatesResponse {
        last_pending_certificate: expected_last_pending_certificates,
    };
    assert_eq!(response, expected_response);

    info!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}

#[rstest]
#[test(tokio::test)]
#[timeout(Duration::from_secs(300))]
async fn test_tce_open_stream_with_checkpoint(
    input_certificates: Vec<CertificateDelivered>,
    #[with(input_certificates.clone())]
    #[future]
    start_node: TceContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = start_node.await;

    let source_subnet_id_1: SubnetId = SubnetId {
        value: SOURCE_SUBNET_ID_1.into(),
    };
    let source_subnet_id_1_stream_position = 4;
    let source_subnet_id_1_prefilled_certificates =
        &input_certificates[0..SOURCE_SUBNET_ID_1_NUMBER_OF_PREFILLED_CERTIFICATES];

    let source_subnet_id_2: SubnetId = SubnetId {
        value: SOURCE_SUBNET_ID_2.into(),
    };
    let source_subnet_id_2_stream_position = 2;
    let source_subnet_id_2_prefilled_certificates =
        &input_certificates[SOURCE_SUBNET_ID_1_NUMBER_OF_PREFILLED_CERTIFICATES..];

    let target_subnet_id: SubnetId = SubnetId {
        value: TARGET_SUBNET_ID_1.into(),
    };

    // Ask for target checkpoint for 2 subnets, one from position 4, other from position 2
    let target_checkpoint = TargetCheckpoint {
        target_subnet_ids: vec![target_subnet_id.clone()],
        positions: vec![
            TargetStreamPosition {
                source_subnet_id: source_subnet_id_1.clone().into(),
                target_subnet_id: target_subnet_id.clone().into(),
                position: source_subnet_id_1_stream_position,
                certificate_id: Some(
                    source_subnet_id_1_prefilled_certificates[3]
                        .certificate
                        .id
                        .into(),
                ),
            },
            TargetStreamPosition {
                source_subnet_id: source_subnet_id_2.clone().into(),
                target_subnet_id: target_subnet_id.clone().into(),
                position: source_subnet_id_2_stream_position,
                certificate_id: Some(
                    source_subnet_id_2_prefilled_certificates[1]
                        .certificate
                        .id
                        .into(),
                ),
            },
        ],
    };

    // Make list of expected certificate, first received certificate for every source subnet and its position
    let mut expected_certs = HashMap::<SubnetId, (Certificate, u64)>::new();
    expected_certs.insert(
        input_certificates[4].certificate.source_subnet_id.into(),
        (input_certificates[4].certificate.clone().into(), 4),
    );
    expected_certs.insert(
        input_certificates[SOURCE_SUBNET_ID_1_NUMBER_OF_PREFILLED_CERTIFICATES + 2]
            .certificate
            .source_subnet_id
            .into(),
        (
            input_certificates[SOURCE_SUBNET_ID_1_NUMBER_OF_PREFILLED_CERTIFICATES + 2]
                .certificate
                .clone()
                .into(),
            2,
        ),
    );

    info!("Prefilled certificates:");
    let mut index = -1;
    input_certificates
        .iter()
        .map(|c| c.certificate.id)
        .collect::<Vec<_>>()
        .iter()
        .for_each(|id| {
            index += 1;
            info!("{index}: {id}")
        });

    //Outbound stream
    let in_stream = async_stream::stream! {
        yield watch_certificates_request::OpenStream {
            target_checkpoint: Some(target_checkpoint),
            source_checkpoint: None
        }.into()
    };

    let response = context
        .api_grpc_client
        .watch_certificates(in_stream)
        .await
        .unwrap();

    let mut resp_stream = response.into_inner();

    info!("TCE client: waiting for watch certificate response");

    while let Some(received) = resp_stream.next().await {
        debug!("TCE client received: {:?}", received);
        let received = received.unwrap();
        match received.event {
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: Some(received_certificate),
                positions,
            })) => {
                if let Some((expected_first_certificate_from_subnet, expected_position)) =
                    expected_certs.get(received_certificate.source_subnet_id.as_ref().unwrap())
                {
                    info!(
                        "\n\nCertificate received: {} source sid {}, target sid {}",
                        received_certificate.id.as_ref().unwrap(),
                        received_certificate.source_subnet_id.as_ref().unwrap(),
                        received_certificate.target_subnets[0]
                    );
                    assert_eq!(
                        received_certificate,
                        *expected_first_certificate_from_subnet
                    );
                    let received_position = positions.get(0).unwrap();
                    assert_eq!(*expected_position, received_position.position);
                    assert_eq!(
                        received_position.target_subnet_id.as_ref().unwrap(),
                        &received_certificate.target_subnets[0]
                    );
                    // First certificate received from source subnet, remove it from the expected list
                    expected_certs.remove(received_certificate.source_subnet_id.as_ref().unwrap());
                    info!(
                        "Received valid first certificate from source subnet {} certificate id {}",
                        received_certificate.source_subnet_id.as_ref().unwrap(),
                        received_certificate.id.as_ref().unwrap(),
                    );
                } else {
                    debug!(
                        "\n\nAdditional certificate received from the source subnet: {} source \
                         sid {}, target sid {}",
                        received_certificate.id.as_ref().unwrap(),
                        received_certificate.source_subnet_id.as_ref().unwrap(),
                        received_certificate.target_subnets[0]
                    );
                }

                if expected_certs.is_empty() {
                    info!("All expected certificates received");
                    break;
                }
            }
            Some(watch_certificates_response::Event::StreamOpened(
                watch_certificates_response::StreamOpened { subnet_ids },
            )) => {
                debug!("TCE client: stream opened for subnet_ids {:?}", subnet_ids);
                continue;
            }
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: None,
                ..
            })) => {
                panic!("TCE client: empty certificate received");
            }
            _ => {
                panic!("TCE client: something unexpected is received");
            }
        }
    }
    info!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}

#[fixture]
fn input_certificates() -> Vec<CertificateDelivered> {
    let mut certificates = Vec::new();
    certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
        SOURCE_SUBNET_ID_1_NUMBER_OF_PREFILLED_CERTIFICATES,
    ));

    certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_2,
        &[TARGET_SUBNET_ID_1],
        SOURCE_SUBNET_ID_2_NUMBER_OF_PREFILLED_CERTIFICATES,
    ));

    certificates
}

#[rstest]
#[test(tokio::test)]
async fn test_tce_proxy_submit_certificate(
    #[future] start_node: TceContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = start_node.await;

    let source_subnet_id = SOURCE_SUBNET_ID_1;
    let target_subnet_stream_positions = Vec::new();

    let mut certificates = Vec::new();
    certificates.append(&mut create_certificate_chain(
        SOURCE_SUBNET_ID_1,
        &[TARGET_SUBNET_ID_1],
        5,
    ));

    // Create tce proxy client
    let (tce_proxy_worker, _source_head_certificate_id) =
        match TceProxyWorker::new(TceProxyConfig {
            subnet_id: source_subnet_id,
            base_tce_api_url: context.api_entrypoint.clone(),
            positions: target_subnet_stream_positions,
        })
        .await
        {
            Ok((tce_proxy_worker, mut source_head_certificate)) => {
                if let Some((cert, _position)) = &mut source_head_certificate {
                    if cert.id == CertificateId::default() {
                        warn!(
                            "Tce has not provided source head certificate, starting from subnet \
                             genesis block..."
                        );
                        source_head_certificate = None;
                    }
                }

                info!(
                    "TCE proxy client is starting for the source subnet {:?} from the head {:?}",
                    source_subnet_id, source_head_certificate
                );
                let source_head_certificate_id =
                    source_head_certificate.map(|(cert, position)| (cert.id, position));
                (tce_proxy_worker, source_head_certificate_id)
            }
            Err(e) => {
                panic!("Unable to create TCE Proxy: {e}");
            }
        };

    for (index, cert) in certificates.into_iter().enumerate() {
        match tce_proxy_worker
            .send_command(TceProxyCommand::SubmitCertificate {
                cert: Box::new(cert.certificate),
                ctx: Default::default(),
            })
            .await
        {
            Ok(_) => {
                info!("Certificate {} successfully submitted", index);
            }
            Err(e) => {
                panic!("Error submitting certificate: {e}");
            }
        }
    }

    // Wait for certificates to be submitted
    tokio::time::sleep(Duration::from_secs(5)).await;

    // TODO: get pending certificates from TCE to make sure they were actually submitted

    info!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}
