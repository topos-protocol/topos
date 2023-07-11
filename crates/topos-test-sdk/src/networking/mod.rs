use std::net::SocketAddr;
use std::net::{TcpListener, TcpStream};

pub fn get_available_port() -> u16 {
    get_available_addr().port()
}

pub fn get_available_addr() -> SocketAddr {
    let host = "127.0.0.1";

    let listener = TcpListener::bind((host, 0)).expect("Can't bind to an available port");
    let addr = listener
        .local_addr()
        .expect("Can't extract local addr from listener");

    // Forcing the port into the TIME_WAIT state is necessary to ensure that the port will be
    // reserved from some limited amount of time (roughly 60s on some Linux systems)
    let _sender = TcpStream::connect(addr).expect("Can't connect to an available port");
    let _incoming = listener.accept().expect("Can't accept connection");

    addr
}
