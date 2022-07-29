use std::convert::Infallible;

use async_trait::async_trait;
use cucumber::{given, then, WorldInit};
use hyper::Client;
use std::process::Child as Process;
use std::process::Command;

/// Path to the binary
const TCE_BIN: &str = env!("CARGO_BIN_EXE_topos-tce-node-app");

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
    user: Option<String>,
    capacity: usize,
}

#[async_trait(?Send)]
impl cucumber::World for World {
    type Error = Infallible;

    async fn new() -> Result<Self, Self::Error> {
        Ok(Self {
            user: None,
            capacity: 0,
        })
    }
}

#[given(expr = "tce node listening {word}")]
async fn launch_tce_node(w: &mut World, port: String) {
    sleep(Duration::from_secs(2)).await;
    // Parameters
    let endpoint = "health_check";
    let ip_address = "localhost";
    let uri = format!("http://{ip_address}:{port}/{endpoint}");

    let args = ["--ram-storage", "--web-api-local-port", port.as_str()];

    // Given
    // Launch the TCE process
    let mut tce_process = Command::new(TCE_BIN)
        .args(args)
        .spawn()
        .expect("launched tce node");

    assert!(is_running(&mut tce_process));

    // When
    // Proceed to the health check
    let resp = Client::new().get(uri.parse::<hyper::Uri>().unwrap()).await;

    // Then
    assert_eq!(resp.unwrap().status(), 200);

    // Kill the process
    tce_process.kill().expect("not launched");

    w.user = Some(port);
}

#[then("she is full")]
async fn is_full(w: &mut World) {
    sleep(Duration::from_secs(2)).await;

    assert_eq!(w.capacity, 3, "{} isn't full!", w.user.as_ref().unwrap());
}

#[tokio::main]
async fn main() {
    World::run("tests/features/readme").await;
}
