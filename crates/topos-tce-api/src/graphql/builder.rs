use std::{net::SocketAddr, sync::Arc};

use async_graphql::{EmptyMutation, Schema};
use async_graphql_axum::GraphQLSubscription;
use axum::{extract::Extension, routing::get, Router, Server};
use http::{header, Method};
use tokio::sync::mpsc;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    graphql::{
        query::{QueryRoot, ServiceSchema},
        routes::{graphql_playground, health},
    },
    runtime::InternalRuntimeCommand,
};
use topos_tce_storage::validator::ValidatorStore;

use super::query::SubscriptionRoot;

#[derive(Default)]
pub struct ServerBuilder {
    store: Option<Arc<ValidatorStore>>,
    serve_addr: Option<SocketAddr>,
    runtime: Option<mpsc::Sender<InternalRuntimeCommand>>,
}

impl ServerBuilder {
    /// Sets the runtime command channel
    ///
    /// Mostly used to manage Transient streams
    pub(crate) fn runtime(mut self, runtime: mpsc::Sender<InternalRuntimeCommand>) -> Self {
        self.runtime = Some(runtime);

        self
    }
    pub(crate) fn store(mut self, store: Arc<ValidatorStore>) -> Self {
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

        let fullnode_store = store.fullnode_store();
        let runtime = self
            .runtime
            .take()
            .expect("Cannot build GraphQL server without the internal runtime channel");

        let schema: ServiceSchema = Schema::build(QueryRoot, EmptyMutation, SubscriptionRoot)
            .data(store)
            .data(fullnode_store)
            .data(runtime)
            .finish();

        let app = Router::new()
            .route(
                "/",
                get(graphql_playground)
                    .post_service(async_graphql_axum::GraphQL::new(schema.clone())),
            )
            .route_service("/ws", GraphQLSubscription::new(schema.clone()))
            .route("/health", get(health))
            .layer(cors)
            .layer(Extension(schema));

        let serve_addr = self.serve_addr.take().expect("Server address is not set");
        Server::bind(&serve_addr).serve(app.into_make_service())
    }
}
