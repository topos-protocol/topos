use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    task::Poll,
};

use libp2p::swarm::{
    handler::{ConnectionEvent, FullyNegotiatedInbound, FullyNegotiatedOutbound},
    ConnectionHandler, ConnectionHandlerEvent, SubstreamProtocol,
};
use tracing::{info, warn};

use self::protocol::GrpcUpgradeProtocol;

use super::{Event, RequestId};

mod protocol;

/// Handler for gRPC connections
pub struct Handler {
    /// Next inbound request id
    inbound_request_id: Arc<AtomicU64>,
    /// Pending events to send
    pending_events: VecDeque<Event>,
    /// Optional outbound request id
    outbound_request_id: Option<RequestId>,
}

impl Handler {
    pub(crate) fn new(inbound_request_id: Arc<AtomicU64>) -> Self {
        Self {
            inbound_request_id,
            pending_events: VecDeque::new(),
            outbound_request_id: None,
        }
    }
}

impl ConnectionHandler for Handler {
    type FromBehaviour = RequestId;

    type ToBehaviour = Event;

    type Error = void::Void;

    type InboundProtocol = GrpcUpgradeProtocol;

    type OutboundProtocol = GrpcUpgradeProtocol;

    type InboundOpenInfo = RequestId;

    type OutboundOpenInfo = RequestId;

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        let id = self.inbound_request_id.fetch_add(1, Ordering::Relaxed);

        SubstreamProtocol::new(GrpcUpgradeProtocol {}, RequestId(id))
    }

    fn connection_keep_alive(&self) -> libp2p::swarm::KeepAlive {
        libp2p::swarm::KeepAlive::Yes
    }

    fn on_behaviour_event(&mut self, id: Self::FromBehaviour) {
        if let Some(prev) = self.outbound_request_id.replace(id) {
            warn!(
                "Received new outbound request id {:?} while previous request id {:?} is still \
                 pending",
                id, prev
            );
        }
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
        match event {
            // New Inbound stream
            ConnectionEvent::FullyNegotiatedInbound(FullyNegotiatedInbound { protocol, info }) => {
                self.pending_events
                    .push_back(Event::InboundNegotiatedStream {
                        request_id: info,
                        stream: protocol,
                    })
            }
            // New Outbound stream
            ConnectionEvent::FullyNegotiatedOutbound(FullyNegotiatedOutbound {
                protocol,
                info,
            }) => self
                .pending_events
                .push_back(Event::OutboundNegotiatedStream {
                    request_id: info,
                    stream: protocol,
                }),
            ConnectionEvent::ListenUpgradeError(_) => todo!(),
            ConnectionEvent::DialUpgradeError(_) => todo!(),
            ConnectionEvent::AddressChange(_) => (),
            ConnectionEvent::LocalProtocolsChange(_) => (),
            ConnectionEvent::RemoteProtocolsChange(_) => (),
        }
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<
        libp2p::swarm::ConnectionHandlerEvent<
            Self::OutboundProtocol,
            Self::OutboundOpenInfo,
            Self::ToBehaviour,
            Self::Error,
        >,
    > {
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(event));
        }

        if let Some(request_id) = self.outbound_request_id.take() {
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(GrpcUpgradeProtocol {}, request_id),
            });
        }

        Poll::Pending
    }
}
