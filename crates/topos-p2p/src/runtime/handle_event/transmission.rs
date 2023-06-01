use libp2p::request_response::{InboundFailure, RequestResponseEvent, RequestResponseMessage};
use tracing::{error, info, warn};

use crate::{
    behaviour::transmission::codec::{TransmissionRequest, TransmissionResponse},
    error::CommandExecutionError,
    Event, Runtime,
};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<RequestResponseEvent<TransmissionRequest, TransmissionResponse>> for Runtime {
    async fn handle(
        &mut self,
        event: RequestResponseEvent<TransmissionRequest, TransmissionResponse>,
    ) {
        match event {
            RequestResponseEvent::Message {
                peer,
                message:
                    RequestResponseMessage::Request {
                        request, channel, ..
                    },
            } => {
                _ = self.event_sender.try_send(Event::TransmissionOnReq {
                    from: peer,
                    data: request.0,
                    channel,
                });
            }

            RequestResponseEvent::Message {
                message:
                    RequestResponseMessage::Response {
                        request_id,
                        response,
                    },
                peer,
            } => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    if sender.send(Ok(response.0)).is_err() {
                        warn!("Could not send response to request {request_id} because initiator is dropped");
                    }
                }
            }
            RequestResponseEvent::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            } => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    if sender.send(Err(error.into())).is_err() {
                        warn!("Could not send RequestFailure for request {request_id} because initiator is dropped");
                    }
                } else {
                    warn!("Received an OutboundRequest failure for an unknown request {request_id} from {peer} because of {:?}", error)
                }
            }

            RequestResponseEvent::ResponseSent { .. } => {}
            // RequestResponseEvent::InboundFailure {
            //     peer,
            //     request_id,
            //     error: InboundFailure::ConnectionClosed,
            // } => {
            //     if let Some(sender) = self.pending_requests.remove(&request_id) {
            //         if sender
            //             .send(Err(CommandExecutionError::ConnectionClosed))
            //             .is_err()
            //         {
            //             warn!("Could not send RequestFailure for request {request_id} because initiator is dropped");
            //         }
            //     } else {
            //         info!("Received an InboundRequest failure for an unknown request {request_id} from {peer} because the connection was closed");
            //     }
            // }
            RequestResponseEvent::InboundFailure {
                peer,
                request_id,
                error,
            } => {
                warn!(
                    "Received an InboundRequest failure for request {request_id} from {peer}: {:?}",
                    error
                );
            }
        }
    }
}
