use crate::behaviour::grpc::RequestId;

use super::ProtocolRequest;

#[derive(Debug)]
pub enum Event {
    InboundNegotiatedStream {
        request_id: RequestId,
        stream: libp2p::Stream,
    },
    OutboundNegotiatedStream {
        request_id: RequestId,
        stream: libp2p::Stream,
    },
    UnsupportedProtocol(RequestId, String),
    OutboundTimeout(ProtocolRequest),
}
