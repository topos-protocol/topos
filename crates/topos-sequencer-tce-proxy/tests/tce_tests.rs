use futures::{Future, StreamExt};
use rstest::*;
use serial_test::serial;
use topos_core::api::tce::v1::{
    watch_certificates_request, watch_certificates_response,
    watch_certificates_response::CertificatePushed, SubmitCertificateRequest,
};
use topos_core::api::uci::v1::Certificate;
pub const CLIENT_SUBNET_ID: &str = "0";

mod common;

#[allow(dead_code)]
struct Context {
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
    let (handle, shutdown_tce_node_signal) = match common::start_mock_tce_service().await {
        Ok(result) => result,
        Err(e) => {
            eprint!("Unable to start mock tce node, details: {}", e);
            panic!();
        }
    };
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    Context {
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

    println!("Creating TCE node client");
    let mut client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        "http://".to_string() + common::TCE_MOCK_NODE_TEST_URL,
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
                initial_subnet_id: "subnet_id".into(),
                cert_id: "id".to_string(),
                prev_cert_id: "previous_id".to_string(),
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

    println!("Creating TCE node client");
    let mut client = match topos_core::api::tce::v1::api_service_client::ApiServiceClient::connect(
        "http://".to_string() + common::TCE_MOCK_NODE_TEST_URL,
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
    let in_stream = async_stream::stream! {
        yield watch_certificates_request::OpenStream { subnet_ids: vec![CLIENT_SUBNET_ID.into()] }.into();
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
                assert_eq!(certificate.cert_id, "1");
                assert_eq!(
                    certificate.initial_subnet_id,
                    common::TCE_MOCK_NODE_SOURCE_SUBNET_ID
                );
                break;
            }
            Some(watch_certificates_response::Event::StreamOpened(
                watch_certificates_response::StreamOpened { subnet_ids },
            )) => {
                println!("TCE client: stream opened for subnet_ids {:?}", subnet_ids);
                assert_eq!(&subnet_ids[0].value, CLIENT_SUBNET_ID);
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
