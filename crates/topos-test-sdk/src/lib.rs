pub mod certificates;

pub mod certificate_producer;
pub mod crypto;
pub mod networking;
pub mod p2p;
pub mod storage;
pub mod tce;

use rand::Rng;
use std::{
    collections::HashSet,
    net::SocketAddr,
    path::PathBuf,
    str::FromStr,
    sync::Mutex,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use lazy_static::lazy_static;
use rstest::fixture;

lazy_static! {
    pub static ref PORT_MAPPING: Mutex<HashSet<u16>> = Mutex::new(HashSet::new());
}

pub mod grpc;

pub mod constants {
    use proc_macro_sdk::generate_certificate_ids;
    use proc_macro_sdk::generate_source_subnet_ids;
    use proc_macro_sdk::generate_target_subnet_ids;
    use topos_core::uci::CertificateId;
    use topos_core::uci::CERTIFICATE_ID_LENGTH;

    generate_source_subnet_ids!(100..150);
    generate_target_subnet_ids!(150..200);

    // Certificate range is 0..100
    pub const PREV_CERTIFICATE_ID: CertificateId =
        CertificateId::from_array([0u8; CERTIFICATE_ID_LENGTH]);
    generate_certificate_ids!(1..100);
}

#[macro_export]
macro_rules! wait_for_event {
    ($node:expr, matches: $( $pattern:pat_param )|+ $( if $guard: expr )?, $error_msg:expr) => {
        wait_for_event!($node, matches: $( $pattern )|+ $( if $guard )?, $error_msg, 100);
    };

    ($node:expr, matches: $( $pattern:pat_param )|+ $( if $guard: expr )?, $error_msg:expr, $timeout:expr) => {
        let assertion = async {
            while let Some(event) = $node.await {
                if matches!(event, $( $pattern )|+ $( if $guard )?) {
                    break;
                }
            }
        };

        if let Err(_) = tokio::time::timeout(std::time::Duration::from_millis($timeout), assertion).await
        {
            panic!("Timed out waiting ({}ms) for event: {}", $timeout, $error_msg);
        }
    };
}

pub fn get_available_port() -> u16 {
    get_available_addr().port()
}
pub fn get_available_addr() -> SocketAddr {
    let mut port_mapping = PORT_MAPPING.lock().unwrap();

    let mut addr = None;
    for _ in 0..10 {
        let new_addr = next_available_port();
        if port_mapping.insert(new_addr.port()) {
            addr = Some(new_addr);
            break;
        }
    }

    assert!(addr.is_some(), "Can't find an available port");
    addr.unwrap()
}

fn next_available_port() -> SocketAddr {
    // let socket = UdpSocket::bind("127.0.0.1:0").expect("Can't find an available port");
    // socket.local_addr().unwrap()
    //
    use std::net::{TcpListener, TcpStream};

    let host = "127.0.0.1";
    // Request a random available port from the OS
    let listener = TcpListener::bind((host, 0)).expect("Can't bind to an available port");
    let addr = listener.local_addr().expect("Can't find an available port");

    // Create and accept a connection (which we'll promptly drop) in order to force the port
    // into the TIME_WAIT state, ensuring that the port will be reserved from some limited
    // amount of time (roughly 60s on some Linux systems)
    let _sender = TcpStream::connect(addr).expect("Can't connect to an available port");
    let _incoming = listener.accept().expect("Can't accept an available port");

    addr
}

#[fixture]
fn folder_name() -> &'static str {
    Box::leak(Box::new(
        thread::current().name().unwrap().replace("::", "_"),
    ))
}

#[fixture]
pub fn create_folder(folder_name: &str) -> PathBuf {
    let dir = env!("TOPOS_TEST_SDK_TMP");
    let mut temp_dir =
        std::path::PathBuf::from_str(dir).expect("Unable to read CARGO_TARGET_TMPDIR");
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let mut rng = rand::thread_rng();

    temp_dir.push(format!(
        "{}/data_{}_{}",
        folder_name,
        time.as_nanos(),
        rng.gen::<u64>()
    ));

    temp_dir
}
