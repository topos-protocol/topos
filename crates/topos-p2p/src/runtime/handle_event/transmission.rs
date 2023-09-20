use libp2p::request_response::{
    Event as RequestResponseEvent, InboundFailure, Message as RequestResponseMessage,
};
use tracing::{error, info, warn};

use crate::{
    behaviour::transmission::codec::{TransmissionRequest, TransmissionResponse},
    error::CommandExecutionError,
    Event, Runtime,
};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<RequestResponseEvent<TransmissionRequest, Result<TransmissionResponse, ()>>>
    for Runtime
{
    async fn handle(
        &mut self,
        event: RequestResponseEvent<TransmissionRequest, Result<TransmissionResponse, ()>>,
    ) {
        match event {
            RequestResponseEvent::Message {
                peer,
                message:
                    RequestResponseMessage::Request {
                        request, channel, ..
                    },
            } => {
                let event = match request {
                    TransmissionRequest::Synchronizer(data) => Event::SynchronizerRequest {
                        from: peer,
                        data,
                        channel,
                    },
                    TransmissionRequest::Transmission(data) => Event::TransmissionOnReq {
                        from: peer,
                        data,
                        channel,
                    },
                };

                _ = self.event_sender.try_send(event);
            }

            RequestResponseEvent::Message {
                message:
                    RequestResponseMessage::Response {
                        request_id,
                        response,
                    },
                peer,
            } => {
                if let Ok(res) = response {
                    if let Some(sender) = self.pending_requests.remove(&request_id) {
                        let data = match res {
                            TransmissionResponse::Transmission(data) => data,
                            TransmissionResponse::Synchronizer(data) => data,
                        };

                        if sender.send(Ok(data)).is_err() {
                            warn!(
                                "Could not send response to request {request_id} because \
                                 initiator is dropped"
                            );
                        }
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
                        warn!(
                            "Could not send RequestFailure for request {request_id} because \
                             initiator is dropped"
                        );
                    }
                } else {
                    warn!(
                        "Received an OutboundRequest failure for an unknown request {request_id} \
                         from {peer} because of {:?}",
                        error
                    )
                }
            }

            RequestResponseEvent::ResponseSent { .. } => {}

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
