use assert_cmd::Command;
use std::{thread, time::Duration};
use topos_core::api::grpc::tce::v1::StatusRequest;
use topos_test_sdk::tce::create_network;

#[test]
fn help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("tce").arg("push-certificate").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(result);

    Ok(())
}

#[test_log::test(tokio::test)]
// FIXME: This test is flaky, it fails sometimes because of sample failure
async fn assert_delivery() -> Result<(), Box<dyn std::error::Error>> {
    let mut peers_context = create_network(10).await;

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

    let nodes: String = peers_context
        .iter()
        .map(|peer| peer.1.api_entrypoint.clone())
        .collect::<Vec<_>>()
        .join(",");

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    _ = thread::spawn(|| {
        let mut cmd = Command::cargo_bin("topos").unwrap();
        cmd.env("TOPOS_LOG_FORMAT", "json");
        cmd.env("RUST_LOG", "topos=debug");

        cmd.arg("tce")
            .arg("push-certificates")
            .args(["-f", "plain"])
            .arg("-n")
            .arg(nodes);

        cmd.assert().success();

        tx.send(()).unwrap();
    });

    _ = tokio::time::timeout(Duration::from_secs(15), rx)
        .await
        .unwrap();

    Ok(())
}
