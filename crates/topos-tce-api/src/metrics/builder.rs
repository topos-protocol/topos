use std::net::SocketAddr;

use topos_metrics::gather_metrics;

use axum::{routing::get, Router, Server};
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
            get(|| async {
                let metrics = gather_metrics();
                let mut buf = String::new();
                let reg = topos_p2p::constant::METRIC_REGISTRY.lock().await;
                _ = prometheus_client::encoding::text::encode(&mut buf, &reg);

                format!("{metrics}{buf}")
            }),
        );

        let serve_addr = self
            .serve_addr
            .take()
            .expect("Metrics server address is not set");
        info!("Starting metrics server on {}", serve_addr);
        Server::bind(&serve_addr).serve(app.into_make_service())
    }
}
