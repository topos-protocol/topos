use crate::support::network::create_network;
use assert_cmd::{prelude::*, Command};
use std::{net::UdpSocket, thread, time::Duration};
use test_log::test;
use tonic::{Request, Response, Status};
use topos_core::api::tce::v1::{
    console_service_server::{ConsoleService, ConsoleServiceServer},
    PushPeerListRequest, PushPeerListResponse, StatusRequest, StatusResponse,
};

mod support {
    pub mod network;
}

#[test]
fn help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("checker").arg("tce").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(result);

    Ok(())
}

#[test]
fn assert_delivery_help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("checker")
        .arg("tce")
        .arg("assert-delivery")
        .arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(result);

    Ok(())
}

#[test_log::test(tokio::test)]
// FIXME: This test is flaky, it fails sometimes because of sample failure
async fn assert_delivery() -> Result<(), Box<dyn std::error::Error>> {
    let mut peers_context = create_network(10, 4).await;

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

    let join = thread::spawn(|| {
        let mut cmd = Command::cargo_bin("topos").unwrap();
        cmd.env("TOPOS_LOG_FORMAT", "json");
        cmd.env("RUST_LOG", "topos=debug");

        cmd.arg("checker")
            .arg("tce")
            .arg("assert-delivery")
            // .args(&["--timeout-broadcast", "15"])
            .args(&["-f", "plain"])
            .arg(nodes);

        let output = cmd.assert().success();

        tx.send(()).unwrap();
    });

    rx.await.unwrap();

    Ok(())
}
