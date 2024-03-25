use tracing::debug;

use crate::{behaviour::grpc, Runtime};

use super::{EventHandler, EventResult};

#[async_trait::async_trait]
impl EventHandler<grpc::Event> for Runtime {
    async fn handle(&mut self, event: grpc::Event) -> EventResult {
        match event {
            grpc::Event::OutboundFailure {
                peer_id,
                request_id,
                error,
            } => {
                debug!(
                    "Outbound connection failure to peer {} for request {}: {}",
                    peer_id, request_id, error
                );
            }
            grpc::Event::OutboundSuccess {
                peer_id,
                request_id,
                ..
            } => {
                debug!(
                    "Outbound connection success to peer {} for request {}",
                    peer_id, request_id
                );
            }
            grpc::Event::InboundNegotiatedConnection {
                request_id,
                connection_id,
            } => {
                debug!(
                    "Inbound connection negotiated for request {} with connection {}",
                    request_id, connection_id
                );
            }
            grpc::Event::OutboundNegotiatedConnection {
                peer_id,
                request_id,
            } => {
                debug!(
                    "Outbound connection negotiated to peer {} for request {}",
                    peer_id, request_id
                );
            }
        }
        Ok(())
    }
}
