mod utils;

use std::{thread, time::Duration};

use assert_cmd::Command;

use rstest::*;
use topos_core::api::grpc::tce::v1::StatusRequest;
use topos_test_sdk::tce::create_network;
use tracing::{error, info};

#[test]
fn help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("tce").arg("push-certificate").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(utils::sanitize_config_folder_path(result));

    Ok(())
}

#[rstest]
#[test_log::test(tokio::test)]
#[timeout(Duration::from_secs(120))]
async fn assert_delivery() -> Result<(), Box<dyn std::error::Error>> {
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

        match topos_test_sdk::integration::check_certificate_delivery(5, peers, 5).await {
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
