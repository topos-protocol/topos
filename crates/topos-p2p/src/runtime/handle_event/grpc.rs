use bytes::BufMut;
use bytes::BytesMut;
use http::HeaderMap;
use http_body::Body;
use http_body_util::BodyExt;
use tokio::io::AsyncWriteExt;
use tonic::client::GrpcService;
use tracing::info;

use crate::{behaviour::grpc, Runtime};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<grpc::Event> for Runtime {
    async fn handle(&mut self, event: grpc::Event) {
        info!("gRPC event: {:?}", event);
    }
}
