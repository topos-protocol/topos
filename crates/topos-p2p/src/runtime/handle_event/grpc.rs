use crate::{behaviour::grpc, Runtime};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<grpc::Event> for Runtime {
    async fn handle(&mut self, _event: grpc::Event) {}
}
