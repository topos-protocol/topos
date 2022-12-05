use assert_cmd::prelude::*;
use futures::FutureExt;
use std::{net::UdpSocket, process::Command, time::Duration};
use tokio::spawn;
use tonic::{Request, Response, Status};
use topos_core::api::tce::v1::{
    console_service_server::{ConsoleService, ConsoleServiceServer},
    PushPeerListRequest, PushPeerListResponse,
};

#[test]
fn help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("tce").arg("run").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(result);

    Ok(())
}

#[tokio::test]
async fn do_not_push_empty_list() -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    let addr = socket.local_addr().unwrap();
    let port = addr.port();

    let server = ConsoleServiceServer::new(DummyServer);

    let grpc = tonic::transport::Server::builder()
        .add_service(server)
        .serve(addr)
        .boxed();

    spawn(grpc);

    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.env("TOPOS_LOG_FORMAT", "json");
    cmd.arg("tce")
        .arg("push-peer-list")
        .arg("1234")
        .arg("--endpoint")
        .arg(format!("http://localhost:{}", port));

    let output = cmd.assert().failure();

    insta::assert_json_snapshot!(serde_json::from_slice::<serde_json::Value>(&output.get_output().stdout).unwrap(), {".timestamp" => "[timestamp]"});

    Ok(())
}

struct DummyServer;

#[tonic::async_trait]
impl ConsoleService for DummyServer {
    async fn push_peer_list(
        &self,
        _request: Request<PushPeerListRequest>,
    ) -> Result<Response<PushPeerListResponse>, Status> {
        unimplemented!()
    }
}
