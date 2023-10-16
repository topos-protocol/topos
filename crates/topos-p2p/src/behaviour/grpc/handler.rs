use std::{
    collections::{HashSet, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    task::Poll,
};

use libp2p::swarm::{
    handler::{ConnectionEvent, DialUpgradeError, FullyNegotiatedInbound, FullyNegotiatedOutbound},
    ConnectionHandler, ConnectionHandlerEvent, KeepAlive, SubstreamProtocol,
};
use tracing::{debug, warn};

use self::protocol::GrpcUpgradeProtocol;

use super::RequestId;

pub(crate) mod event;
use event::Event;
pub(crate) mod protocol;

#[derive(Debug)]
pub struct ProtocolRequest {
    pub(crate) request_id: RequestId,
    pub(crate) protocol: String,
}

/// Handler for gRPC connections
pub struct Handler {
    /// Next inbound request id
    inbound_request_id: Arc<AtomicU64>,
    /// Pending events to send
    pending_events: VecDeque<Event>,
    /// Optional outbound request id
    outbound_request_id: Option<ProtocolRequest>,
    protocols: HashSet<String>,
    keep_alive: KeepAlive,
}

impl Handler {
    pub(crate) fn new(inbound_request_id: Arc<AtomicU64>, protocols: HashSet<String>) -> Self {
        Self {
            inbound_request_id,
            pending_events: VecDeque::new(),
            outbound_request_id: None,
            protocols,
            keep_alive: KeepAlive::Yes,
        }
    }
}

impl ConnectionHandler for Handler {
    type FromBehaviour = ProtocolRequest;

    type ToBehaviour = event::Event;

    type Error = void::Void;

    type InboundProtocol = GrpcUpgradeProtocol;

    type OutboundProtocol = GrpcUpgradeProtocol;

    type InboundOpenInfo = RequestId;

    type OutboundOpenInfo = ProtocolRequest;

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        let id = self.inbound_request_id.fetch_add(1, Ordering::Relaxed);

        SubstreamProtocol::new(
            GrpcUpgradeProtocol {
                protocols: self.protocols.clone(),
            },
            RequestId(id),
        )
    }

    fn connection_keep_alive(&self) -> libp2p::swarm::KeepAlive {
        self.keep_alive
    }

    fn on_behaviour_event(&mut self, request: Self::FromBehaviour) {
        let request_id = request.request_id;
        if let Some(prev) = self.outbound_request_id.replace(request) {
            warn!(
                "Received new outbound request id {:?} while previous request id {:?} is still \
                 pending",
                request_id, prev.request_id
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
                    request_id: info.request_id,
                    stream: protocol,
                }),
            ConnectionEvent::DialUpgradeError(DialUpgradeError {
                info,
                error: libp2p::swarm::StreamUpgradeError::Timeout,
            }) => {
                self.pending_events.push_back(Event::OutboundTimeout(info));

                // Closing the connection handler
                self.keep_alive = KeepAlive::No;
            }
            ConnectionEvent::DialUpgradeError(DialUpgradeError {
                info,
                error: libp2p::swarm::StreamUpgradeError::NegotiationFailed,
            }) => {
                self.pending_events
                    .push_back(Event::UnsupportedProtocol(info.request_id, info.protocol));

                // Closing the connection handler
                self.keep_alive = KeepAlive::No;
            }
            ConnectionEvent::DialUpgradeError(_)
            | ConnectionEvent::AddressChange(_)
            | ConnectionEvent::ListenUpgradeError(_)
            | ConnectionEvent::LocalProtocolsChange(_)
            | ConnectionEvent::RemoteProtocolsChange(_) => (),
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

        if let Some(request) = self.outbound_request_id.take() {
            debug!(
                "Starting outbound request SubstreamProtocol for {}",
                request.request_id
            );
            let mut protocols = self.protocols.clone();
            protocols.insert(request.protocol.clone());
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(GrpcUpgradeProtocol { protocols }, request),
            });
        }

        Poll::Pending
    }
}
