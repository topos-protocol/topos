use futures::{Future, StreamExt};
use rstest::*;
use serial_test::serial;
use std::path::PathBuf;
use std::str::FromStr;
use test_log::test;
use tokio::{
    sync::{mpsc, oneshot},
    time::Duration,
};
use topos_core::api::shared::v1::{checkpoints::TargetCheckpoint, positions::TargetStreamPosition};
use topos_core::api::shared::v1::{CertificateId, StarkProof, SubnetId};
use topos_core::api::tce::v1::{
    watch_certificates_request, watch_certificates_response,
    watch_certificates_response::CertificatePushed, GetSourceHeadRequest, GetSourceHeadResponse,
    SourceStreamPosition, SubmitCertificateRequest,
};
use topos_core::api::uci::v1::Certificate;
use tracing::{debug, error, info};

mod common;

const TCE_NODE_STARTUP_DELAY: Duration = Duration::from_secs(5);

#[allow(dead_code)]
struct Context {
    endpoint: String,
    shutdown_tce_node_signal: mpsc::Sender<oneshot::Sender<()>>,
    rocksdb_dir: PathBuf,
    prefilled_certificates: Option<Vec<Certificate>>,
}

impl Context {
    pub async fn shutdown(self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Context performing shutdown...");
        let (shutdown_finished_sender, shutdown_finished_receiver) = oneshot::channel::<()>();
        self.shutdown_tce_node_signal
            .send(shutdown_finished_sender)
            .await
            .unwrap();

        shutdown_finished_receiver.await.unwrap();
        info!("Shutdown finished...");

        Ok(())
    }
}

#[fixture]
async fn context_running_tce_test_node() -> Context {
    // Generate rocksdb path
    let mut rocksdb_dir =
        PathBuf::from_str(env!("CARGO_TARGET_TMPDIR")).expect("Unable to read CARGO_TARGET_TMPDIR");
    rocksdb_dir.push(format!(
        "./topos-sequencer-tce-proxy/data_{}/rocksdb",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid system time duration")
            .as_millis()
            .to_string()
    ));

    let (shutdown_tce_node_signal, endpoint) =
        match common::start_tce_test_service(rocksdb_dir.clone()).await {
            Ok(result) => result,
            Err(e) => {
                panic!("Unable to start mock tce node, details: {e}");
            }
        };

    tokio::time::sleep(TCE_NODE_STARTUP_DELAY).await;

    Context {
        endpoint,
        shutdown_tce_node_signal,
        rocksdb_dir,
        prefilled_certificates: None,
    }
}

#[fixture]
async fn context_running_tce_test_node_with_filled_db() -> Context {
    // Generate rocksdb path
    let mut rocksdb_dir =
        PathBuf::from_str(env!("CARGO_TARGET_TMPDIR")).expect("Unable to read CARGO_TARGET_TMPDIR");
    rocksdb_dir.push(format!(
        "./topos-sequencer-tce-proxy/data_{}/rocksdb",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid system time duration")
            .as_millis()
            .to_string()
    ));

    info!("Rocks db path is {rocksdb_dir:?}");

    let certificates = match common::populate_test_database(&rocksdb_dir).await {
        Ok(certificates) => certificates,
        Err(e) => {
            panic!("Unable to start mock tce node with database, details: {e}");
        }
    };

    let (shutdown_tce_node_signal, endpoint) =
        match common::start_tce_test_service(rocksdb_dir.clone()).await {
            Ok(result) => result,
            Err(e) => {
                panic!("Unable to start mock tce node with database, details: {e}");
            }
        };

    tokio::time::sleep(TCE_NODE_STARTUP_DELAY).await;

    Context {
        endpoint,
        shutdown_tce_node_signal,
        rocksdb_dir,
        prefilled_certificates: Some(certificates.into_iter().map(|cert| cert.into()).collect()),
    }
}

#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_run_tce_node(
    context_running_tce_test_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_tce_test_node.await;

    info!("Creating TCE node client");
    let _client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        format!("http://{}", context.endpoint),
    )
    .await
    {
        Ok(client) => client,
        Err(e) => {
            error!("Unable to create client, error: {}", e);
            return Err(Box::from(e));
        }
    };

    info!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}

#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_tce_submit_certificate(
    context_running_tce_test_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_tce_test_node.await;

    let source_subnet_id: SubnetId = [1u8; 32].into();
    let prev_certificate_id: CertificateId = [01u8; 32].into();
    let certificate_id: CertificateId = [02u8; 32].into();

    info!("Creating TCE node client");
    let mut client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        format!("http://{}", context.endpoint),
    )
    .await
    {
        Ok(client) => client,
        Err(e) => {
            error!("Unable to create client, error: {}", e);
            return Err(Box::from(e));
        }
    };

    match client
        .submit_certificate(SubmitCertificateRequest {
            certificate: Some(Certificate {
                source_subnet_id: Some(source_subnet_id.clone()),
                id: Some(certificate_id),
                prev_id: Some(prev_certificate_id),
                target_subnets: vec![],
                state_root: [0u8; 32].to_vec(),
                tx_root_hash: [0u8; 32].to_vec(),
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
            error!("Unable to submit certificate, details: {e:?}");
            return Err(Box::from(e));
        }
    };
    info!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}

#[rstest]
#[test(tokio::test)]
#[serial]
async fn test_tce_watch_certificates(
    context_running_tce_test_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_tce_test_node.await;

    let source_subnet_id: SubnetId = SubnetId {
        value: [1u8; 32].to_vec(),
    };

    info!("Creating TCE node client");
    let mut client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        format!("http://{}", context.endpoint),
    )
    .await
    {
        Ok(client) => client,
        Err(e) => {
            error!("Unable to create client, error: {}", e);
            return Err(Box::from(e));
        }
    };

    //Outbound stream
    let subnet_id_instream = source_subnet_id.clone();
    let in_stream = async_stream::stream! {
        yield watch_certificates_request::OpenStream {
            target_checkpoint: Some(TargetCheckpoint {
                target_subnet_ids: vec![ subnet_id_instream.into() ],
                positions: Vec::new()
            }),
            source_checkpoint: None
        }.into()
    };
    let response = client.watch_certificates(in_stream).await.unwrap();
    let mut resp_stream = response.into_inner();
    info!("TCE client: waiting for watch certificate response");
    while let Some(received) = resp_stream.next().await {
        info!("TCE client received: {:?}", received);
        let received = received.unwrap();
        match received.event {
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: Some(certificate),
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
#[serial]
async fn test_tce_get_source_head_certificate(
    context_running_tce_test_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_tce_test_node.await;

    let source_subnet_id: SubnetId = [1u8; 32].into();
    let default_cert_id: CertificateId = [0u8; 32].into();
    let certificate_id: CertificateId = [02u8; 32].into();

    info!("Creating TCE node client");
    let mut client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        format!("http://{}", context.endpoint),
    )
    .await
    {
        Ok(client) => client,
        Err(e) => {
            error!("Unable to create client, error: {}", e);
            return Err(Box::from(e));
        }
    };

    // Test get source head certificate for empty TCE history
    // This will be actual genesis certificate
    let response = client
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
        verifier: 0,
        proof: Some(StarkProof { value: Vec::new() }),
        signature: Some(Default::default()),
    };
    let expected_response = GetSourceHeadResponse {
        certificate: Some(expected_default_genesis_certificate.clone()),
        position: Some(SourceStreamPosition {
            subnet_id: Some(source_subnet_id.clone()),
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
        verifier: 0,
        proof: Some(StarkProof { value: Vec::new() }),
        signature: Some(Default::default()),
    };
    match client
        .submit_certificate(SubmitCertificateRequest {
            certificate: Some(test_certificate.clone()),
        })
        .await
        .map(|r| r.into_inner())
    {
        Ok(response) => {
            debug!("Certificate successfully submitted {:?}", response);
        }
        Err(e) => {
            error!("Unable to submit certificate, details: {e:?}");
            return Err(Box::from(e));
        }
    };

    // Test get source head certificate for non empty certificate history
    let response = client
        .get_source_head(GetSourceHeadRequest {
            subnet_id: Some(source_subnet_id.clone()),
        })
        .await
        .map(|r| r.into_inner())
        .unwrap();

    // TODO currently only delivered certificates are counted as
    // head source certificate, so default certificate is expected
    // Should be updated to count also pending certificates
    let expected_response = GetSourceHeadResponse {
        certificate: Some(expected_default_genesis_certificate.clone()),
        position: Some(SourceStreamPosition {
            subnet_id: Some(source_subnet_id.clone()),
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
#[serial]
#[ignore = "tce needs to implement streaming from checkpoint"]
async fn test_tce_open_stream_with_checkpoint(
    context_running_tce_test_node_with_filled_db: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_tce_test_node_with_filled_db.await;

    let source_subnet_id: SubnetId = common::SOURCE_SUBNET_ID.into();
    let target_subnet_id: SubnetId = common::TARGET_SUBNET_ID.into();

    info!("Creating TCE node client");
    let mut client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        format!("http://{}", context.endpoint),
    )
    .await
    {
        Ok(client) => client,
        Err(e) => {
            error!("Unable to create client, error: {}", e);
            return Err(Box::from(e));
        }
    };

    let target_checkpoint = TargetCheckpoint {
        target_subnet_ids: vec![target_subnet_id.clone()],
        positions: vec![TargetStreamPosition {
            source_subnet_id: source_subnet_id.clone().into(),
            target_subnet_id: target_subnet_id.clone().into(),
            position: 4,
            certificate_id: context.prefilled_certificates.as_ref().unwrap()[3]
                .id
                .clone(),
        }
        .into()],
    };

    //Outbound stream
    let in_stream = async_stream::stream! {
        yield watch_certificates_request::OpenStream {
            target_checkpoint: Some(target_checkpoint),
            source_checkpoint: None
        }.into()
    };
    let response = client.watch_certificates(in_stream).await.unwrap();
    let mut resp_stream = response.into_inner();
    info!("TCE client: waiting for watch certificate response");
    while let Some(received) = resp_stream.next().await {
        info!("TCE client received: {:?}", received);
        let received = received.unwrap();
        match received.event {
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: Some(certificate),
            })) => {
                info!("Certificate received {:?}", certificate);
                assert_eq!(
                    context.prefilled_certificates.as_ref().unwrap()[4].id,
                    certificate.id
                );
            }
            Some(watch_certificates_response::Event::StreamOpened(
                watch_certificates_response::StreamOpened { subnet_ids },
            )) => {
                debug!("TCE client: stream opened for subnet_ids {:?}", subnet_ids);
                assert_eq!(subnet_ids[0].value, target_subnet_id.value);
                // We have opened connection and 2 way stream, finishing test
                break;
            }
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: None,
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
