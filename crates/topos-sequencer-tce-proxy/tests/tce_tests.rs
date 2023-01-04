use futures::{Future, StreamExt};
use rstest::*;
use serial_test::serial;
use topos_core::api::shared::v1::{CertificateId, SubnetId};
use topos_core::api::tce::v1::{
    watch_certificates_request, watch_certificates_response,
    watch_certificates_response::CertificatePushed, SubmitCertificateRequest,
};
use topos_core::api::uci::v1::Certificate;

mod common;

#[allow(dead_code)]
struct Context {
    endpoint: String,
    service_handle: tokio::task::JoinHandle<()>,
    shutdown_tce_node_signal: futures::channel::oneshot::Sender<()>,
}

impl Context {
    pub async fn shutdown(self) -> Result<(), Box<dyn std::error::Error>> {
        self.shutdown_tce_node_signal.send(()).unwrap();
        Ok(())
    }
}

#[fixture]
async fn context_running_tce_mock_node() -> Context {
    println!("Starting TCE node mock service...");
    let (handle, shutdown_tce_node_signal, endpoint) = match common::start_mock_tce_service().await
    {
        Ok(result) => result,
        Err(e) => {
            eprint!("Unable to start mock tce node, details: {}", e);
            panic!();
        }
    };
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    Context {
        endpoint,
        service_handle: handle,
        shutdown_tce_node_signal,
    }
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_tce_submit_certificate(
    context_running_tce_mock_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_tce_mock_node.await;

    let source_subnet_id: SubnetId = [1u8; 32].into();
    let prev_certificate_id: CertificateId = [01u8; 32].into();
    let certificate_id: CertificateId =  [02u8; 32].into();

    println!("Creating TCE node client");
    let mut client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        format!("http://{}", context.endpoint),
    )
    .await
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Unable to create client, error: {}", e);
            return Err(Box::from(e));
        }
    };

    match client
        .submit_certificate(SubmitCertificateRequest {
            certificate: Some(Certificate {
                source_subnet_id: Some(source_subnet_id),
                id: Some(certificate_id),
                prev_id: Some(prev_certificate_id),
                calls: vec![],
            }),
        })
        .await
        .map(|r| r.into_inner())
    {
        Ok(response) => {
            println!("Certificate successfully submitted {:?}", response);
        }
        Err(e) => {
            eprintln!("Unable to submit certificate");
            return Err(Box::from(e));
        }
    };
    println!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_tce_watch_certificates(
    context_running_tce_mock_node: impl Future<Output = Context>,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = context_running_tce_mock_node.await;

    let source_subnet_id: SubnetId = SubnetId {
        value: [1u8; 32].to_vec(),
    };

    let expected_certificate = CertificateId {
        value: [
            1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ]
        .to_vec(),
    };

    println!("Creating TCE node client");
    let mut client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        format!("http://{}", context.endpoint),
    )
    .await
    {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Unable to create client, error: {}", e);
            return Err(Box::from(e));
        }
    };

    //Outbound stream
    let subnet_id_instream = source_subnet_id.clone();
    let in_stream = async_stream::stream! {
        yield watch_certificates_request::OpenStream { subnet_ids: vec![ subnet_id_instream] }.into();
    };
    let response = client.watch_certificates(in_stream).await.unwrap();
    let mut resp_stream = response.into_inner();
    println!("TCE client: waiting for watch certificate response");
    while let Some(received) = resp_stream.next().await {
        println!("TCE client received: {:?}", received);
        let received = received.unwrap();
        match received.event {
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: Some(certificate),
            })) => {
                println!("Certificate received {:?}", certificate);
                assert_eq!(certificate.id, Some(expected_certificate));
                assert_eq!(certificate.source_subnet_id, Some(source_subnet_id));
                break;
            }
            Some(watch_certificates_response::Event::StreamOpened(
                watch_certificates_response::StreamOpened { subnet_ids },
            )) => {
                println!("TCE client: stream opened for subnet_ids {:?}", subnet_ids);
                assert_eq!(subnet_ids[0].value, source_subnet_id.value);
            }
            Some(watch_certificates_response::Event::CertificatePushed(CertificatePushed {
                certificate: None,
            })) => {
                eprintln!("TCE client: empty certificate received");
                panic!("TCE client: empty certificate received");
            }
            _ => {
                eprintln!("TCE client: something unexpected is received");
                panic!("None received");
            }
        }
    }
    println!("Shutting down TCE node client");
    context.shutdown().await?;
    Ok(())
}
