use crate::{behaviour::grpc, Runtime};

use super::{EventHandler, EventResult};

#[async_trait::async_trait]
impl EventHandler<grpc::Event> for Runtime {
    async fn handle(&mut self, _event: grpc::Event) -> EventResult {
        Ok(())
    }
}
