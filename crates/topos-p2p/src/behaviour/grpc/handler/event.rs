use crate::behaviour::grpc::RequestId;

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
}
