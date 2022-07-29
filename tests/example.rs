use std::convert::Infallible;

use async_trait::async_trait;
use cucumber::{given, then, WorldInit};
use hyper::Client;
use std::process::Child as Process;
use std::process::Command;

/// Path to the binary
const TCE_BIN: &str = env!("CARGO_BIN_EXE_topos-tce-node-app");

/// Trigger graceful termination to given process
fn shutdown(process: &Process) {
    Command::new("kill")
        .args(["-s", "TERM", process.id().to_string().as_str()])
        .spawn()
        .expect("process termination");
}

/// Check whether given process succeeded to start
fn is_running(process: &mut Process) -> bool {
    std::thread::sleep(std::time::Duration::from_secs(1));
    match process.try_wait() {
        Ok(Some(status)) => {
            println!("unable to launch {status}");
        }
        Ok(None) => {}
        Err(e) => println!("error attempting to wait: {e}"),
    }
    process.try_wait().is_ok()
}

#[derive(Debug, WorldInit)]
struct World {
    port: Option<String>,
    tce_process: Option<Process>,
}

#[async_trait(?Send)]
impl cucumber::World for World {
    type Error = Infallible;

    async fn new() -> Result<Self, Self::Error> {
        Ok(Self {
            port: None,
            tce_process: None,
        })
    }
}

#[given(expr = "tce node listening {word}")]
async fn launch_tce_node(w: &mut World, port: String) {
    let args = ["--ram-storage", "--web-api-local-port", port.as_str()];

    // Given
    // Launch the TCE process
    let mut tce_process = Command::new(TCE_BIN)
        .args(args)
        .spawn()
        .expect("launched tce node");

    assert!(is_running(&mut tce_process));

    // Persist the state
    w.port = Some(port);
    w.tce_process = Some(tce_process);
}

#[then(expr = "request to {word} returns status {int}")]
async fn health_check(w: &mut World, endpoint: String, code: u16) {
    // Parameters
    let ip_address = "localhost";
    let uri = format!(
        "http://{ip_address}:{}/{}",
        w.port.as_ref().unwrap(),
        endpoint
    );

    // Proceed to the health check
    let resp = Client::new().get(uri.parse::<hyper::Uri>().unwrap()).await;
    assert_eq!(resp.unwrap().status(), code);

    // Terminate
    // TODO: kill needs to be in `After` hook, looks unstable on cucumber-rs
    shutdown(w.tce_process.as_ref().unwrap());
}

#[tokio::main]
async fn main() {
    World::run("tests/features").await;
}
