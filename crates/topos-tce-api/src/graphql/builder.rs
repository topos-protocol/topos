use std::{net::SocketAddr, sync::Arc};

use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use axum::{extract::Extension, routing::get, Router, Server};
use http::{header, Method};
use hyper;
use tower_http::cors::{Any, CorsLayer};

use crate::graphql::{
    query::{QueryRoot, ServiceSchema},
    routes::{graphql_handler, graphql_playground, health},
};
use topos_tce_storage::fullnode::FullNodeStore;

#[derive(Default)]
pub struct ServerBuilder {
    store: Option<Arc<FullNodeStore>>,
    serve_addr: Option<SocketAddr>,
}

impl ServerBuilder {
    pub(crate) fn store(mut self, store: Arc<FullNodeStore>) -> Self {
        self.store = Some(store);

        self
    }

    pub(crate) fn serve_addr(mut self, addr: Option<SocketAddr>) -> Self {
        self.serve_addr = addr;

        self
    }

    pub async fn build(
        mut self,
    ) -> Server<hyper::server::conn::AddrIncoming, axum::routing::IntoMakeService<Router>> {
        let cors = CorsLayer::new()
            // allow `GET` and `POST` when accessing the resource
            .allow_methods([Method::GET, Method::POST])
            // allow 'application/json' requests
            .allow_headers([header::CONTENT_TYPE])
            // allow requests from any origin
            .allow_origin(Any);

        let store = self
            .store
            .take()
            .expect("Cannot build GraphQL server without a FullNode store");

        let schema: ServiceSchema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
            .data(store)
            .finish();

        let app = Router::new()
            .route("/", get(graphql_playground).post(graphql_handler))
            .route("/health", get(health))
            .layer(cors)
            .layer(Extension(schema));

        let serve_addr = self.serve_addr.take().expect("Server address is not set");
        Server::bind(&serve_addr).serve(app.into_make_service())
    }
}
