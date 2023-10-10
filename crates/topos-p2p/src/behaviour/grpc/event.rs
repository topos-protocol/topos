use libp2p::{swarm::ConnectionId, PeerId};
use tonic::transport::Channel;

use super::{OutboundError, RequestId};

#[derive(Debug)]
pub enum Event {
    OutboundFailure {
        peer_id: PeerId,
        request_id: RequestId,
        error: OutboundError,
    },

    OutboundSuccess {
        peer_id: PeerId,
        request_id: RequestId,
        channel: Channel,
    },

    // InboundNegotiatedStream {
    //     request_id: RequestId,
    //     stream: libp2p::Stream,
    // },
    InboundNegotiatedConnection {
        request_id: RequestId,
        connection_id: ConnectionId,
    },

    // OutboundNegotiatedStream {
    //     request_id: RequestId,
    //     stream: libp2p::Stream,
    // },
    OutboundNegotiatedConnection {
        peer_id: PeerId,
        request_id: RequestId,
    },
}
