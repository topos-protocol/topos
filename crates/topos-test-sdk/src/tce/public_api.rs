use std::net::SocketAddr;

use futures::Stream;
use topos_tce_api::RuntimeClient;
use topos_tce_api::RuntimeEvent;
use topos_tce_storage::StorageClient;

pub async fn create_public_api<A: Into<SocketAddr>>(
    addr: A,
    storage_client: StorageClient,
) -> (RuntimeClient, impl Stream<Item = RuntimeEvent>) {
    topos_tce_api::Runtime::builder()
        .serve_addr(addr.into())
        .storage(storage_client.clone())
        .build_and_launch()
        .await
}
