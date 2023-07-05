use std::str::FromStr;

use futures::Stream;
use rstest::*;
use tonic::transport::{channel, Channel};

use topos_core::api::grpc::tce::v1::{
    api_service_client::ApiServiceClient, console_service_client::ConsoleServiceClient,
};
use topos_tce_api::RuntimeClient;
use topos_tce_api::RuntimeContext;
use topos_tce_api::RuntimeEvent;
use topos_tce_storage::StorageClient;
use tracing::warn;

use crate::networking::get_available_addr;
use crate::storage::storage_client;
use crate::PORT_MAPPING;

pub struct PublicApiContext {
    pub entrypoint: String,
    pub client: RuntimeClient,
    pub api_client: ApiServiceClient<Channel>,
    pub console_client: ConsoleServiceClient<Channel>,
    pub api_context: Option<RuntimeContext>,
}

#[fixture]
pub async fn create_public_api(
    #[future] storage_client: StorageClient,
) -> (PublicApiContext, impl Stream<Item = RuntimeEvent>) {
    let storage_client = storage_client.await;
    let grpc_addr = get_available_addr();
    let graphql_addr = get_available_addr();
    let metrics_addr = get_available_addr();

    let api_port = grpc_addr.port();

    let api_endpoint = format!("http://0.0.0.0:{api_port}");
    warn!("API endpoint: {}", api_endpoint);
    warn!("gRPC endpoint: {}", grpc_addr);
    warn!("GraphQL endpoint: {}", graphql_addr);
    warn!("Metrics endpoint: {}", metrics_addr);
    warn!("PORT MAPPING: {:?}", PORT_MAPPING.lock().unwrap());
    let (client, stream, ctx) = topos_tce_api::Runtime::builder()
        .serve_grpc_addr(grpc_addr)
        .serve_graphql_addr(graphql_addr)
        .serve_metrics_addr(metrics_addr)
        .storage(storage_client)
        .build_and_launch()
        .await;

    let api_channel = channel::Endpoint::from_str(&api_endpoint)
        .unwrap()
        .connect_lazy();

    let console_channel = channel::Endpoint::from_str(&api_endpoint)
        .unwrap()
        .connect_lazy();

    let api_client = ApiServiceClient::new(api_channel);
    let console_client = ConsoleServiceClient::new(console_channel);

    let context = PublicApiContext {
        entrypoint: api_endpoint,
        client,
        api_client,
        console_client,
        api_context: Some(ctx),
    };

    (context, stream)
}
