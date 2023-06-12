use std::net::SocketAddr;
use std::net::UdpSocket;
use std::str::FromStr;

use futures::Stream;
use rstest::*;
use tonic::transport::{channel, Channel};

use topos_core::api::grpc::tce::v1::{
    api_service_client::ApiServiceClient, console_service_client::ConsoleServiceClient,
};
use topos_tce_api::RuntimeClient;
use topos_tce_api::RuntimeEvent;
use topos_tce_storage::StorageClient;

use crate::storage::storage_client;

pub struct PublicApiContext {
    pub entrypoint: String,
    pub client: RuntimeClient,
    pub api_client: ApiServiceClient<Channel>,
    pub console_client: ConsoleServiceClient<Channel>,
}

#[fixture]
fn default_public_api_addr() -> SocketAddr {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    socket.local_addr().expect("Can't extract local_addr")
}

#[fixture]
fn default_public_graphql_addr() -> SocketAddr {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("Can't find an available port");
    socket.local_addr().expect("Can't extract local_addr")
}

#[fixture]
pub async fn create_public_api(
    #[future] storage_client: StorageClient,
    default_public_api_addr: SocketAddr,
    default_public_graphql_addr: SocketAddr,
) -> (PublicApiContext, impl Stream<Item = RuntimeEvent>) {
    let storage_client = storage_client.await;
    let grpc_addr = default_public_api_addr;
    let api_port = grpc_addr.port();

    let graphql_addr = default_public_graphql_addr;

    let api_endpoint = format!("http://0.0.0.0:{api_port}");
    let (client, stream) = topos_tce_api::Runtime::builder()
        .serve_grpc_addr(grpc_addr)
        .serve_graphql_addr(graphql_addr)
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
    };

    (context, stream)
}
