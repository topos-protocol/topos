mod utils;

use std::process::Command;

use assert_cmd::prelude::*;
use tonic::{Request, Response, Status};

use topos_core::api::grpc::tce::v1::{
    console_service_server::ConsoleService, StatusRequest, StatusResponse,
};

#[test]
fn help_display() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("tce").arg("run").arg("-h");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(utils::sanitize_config_folder_path(result));

    Ok(())
}

#[tokio::test]
async fn can_get_a_peer_id_from_a_seed() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("topos")?;
    cmd.arg("tce").arg("keys").arg("--from-seed").arg("1");

    let output = cmd.assert().success();

    let result: &str = std::str::from_utf8(&output.get_output().stdout)?;

    insta::assert_snapshot!(result);

    Ok(())
}

struct DummyServer;

#[tonic::async_trait]
impl ConsoleService for DummyServer {
    async fn status(&self, _: Request<StatusRequest>) -> Result<Response<StatusResponse>, Status> {
        unimplemented!()
    }
}
