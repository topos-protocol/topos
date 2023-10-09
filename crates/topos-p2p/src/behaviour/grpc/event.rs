use libp2p::PeerId;
use tonic::transport::Channel;

use super::{OutboundError, RequestId};

#[derive(Debug)]
pub enum Event {
    OutboundFailure {
        peer_id: PeerId,
        request_id: RequestId,
        error: OutboundError,
    },

    InboundNegotiatedStream {
        request_id: RequestId,
        stream: libp2p::Stream,
    },

    OutboundNegotiatedStream {
        request_id: RequestId,
        stream: libp2p::Stream,
    },
    OutboundSuccess {
        peer_id: PeerId,
        request_id: RequestId,
        channel: Channel,
    },
}
