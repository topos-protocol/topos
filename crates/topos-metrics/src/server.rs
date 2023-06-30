use std::net::SocketAddr;

use axum::{routing::get, Router, Server};
use prometheus::{self, Encoder, TextEncoder};
use tracing::info;

#[derive(Default)]
pub struct ServerBuilder {
    serve_addr: Option<SocketAddr>,
}

impl ServerBuilder {
    pub fn serve_addr(mut self, addr: Option<SocketAddr>) -> Self {
        self.serve_addr = addr;

        self
    }

    pub async fn build(
        mut self,
    ) -> Server<hyper::server::conn::AddrIncoming, axum::routing::IntoMakeService<Router>> {
        let app = Router::new().route(
            "/metrics",
            get(|| async move {
                let mut buffer = Vec::new();
                let encoder = TextEncoder::new();

                // Gather the metrics.
                let metric_families = prometheus::gather();
                // Encode them to send.
                encoder.encode(&metric_families, &mut buffer).unwrap();

                String::from_utf8(buffer.clone()).unwrap()
            }),
        );

        let serve_addr = self.serve_addr.take().expect("Server address is not set");
        info!("Starting metrics server on {}", serve_addr);
        Server::bind(&serve_addr).serve(app.into_make_service())
    }
}
