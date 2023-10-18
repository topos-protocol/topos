use async_graphql::http::GraphiQLSource;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
struct Health {
    healthy: bool,
}

pub(crate) async fn health() -> impl IntoResponse {
    let health = Health { healthy: true };
    (StatusCode::OK, Json(health))
}

/// Build a GraphQL playground
pub async fn graphql_playground() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("/")
            .subscription_endpoint("/ws")
            .finish(),
    )
}
