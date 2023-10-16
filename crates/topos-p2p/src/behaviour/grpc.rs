use std::{
    collections::{HashMap, HashSet, VecDeque},
    io,
    sync::{atomic::AtomicU64, Arc},
    task::{Context, Poll},
};

use futures::{future::BoxFuture, stream::FuturesUnordered, FutureExt, StreamExt};
use handler::Handler;
use libp2p::{
    core::ConnectedPoint,
    swarm::{
        derive_prelude::{ConnectionEstablished, ListenerId, NewListener},
        dial_opts::DialOpts,
        ConnectionClosed, DialError, DialFailure, FromSwarm, NetworkBehaviour, ToSwarm,
    },
    Multiaddr, PeerId,
};
use smallvec::SmallVec;
use std::fmt::Display;
use tokio::sync::{mpsc, oneshot};
use tonic::transport::{server::Router, Channel};
use tracing::{debug, info, warn};

use crate::GrpcRouter;

use self::{
    connection::{
        Connection, OutboundConnectedConnection, OutboundConnection, OutboundConnectionRequest,
    },
    error::OutboundError,
    handler::ProtocolRequest,
    stream::GrpcStream,
};
pub(crate) use event::Event;

pub(crate) mod connection;
pub mod error;
pub mod event;
pub(crate) mod handler;
mod proxy;
mod stream;

#[derive(Default)]
pub struct GrpcContext {
    server: Option<GrpcRouter>,
    client: HashSet<String>,
}

impl GrpcContext {
    pub(crate) fn into_parts(mut self) -> (Option<Router>, (HashSet<String>, HashSet<String>)) {
        let (server, inbound_protocols) = self
            .server
            .map(|server| (Some(server.server), server.protocols))
            .unwrap_or((None, HashSet::new()));

        if self.client.is_empty() {
            self.client = inbound_protocols.clone();
        }

        (server, (inbound_protocols, self.client))
    }

    pub fn with_router(mut self, router: GrpcRouter) -> Self {
        self.server = Some(router);

        self
    }

    pub fn add_client_protocol<S: ToString>(mut self, protocol: S) -> Self {
        self.client.insert(protocol.to_string());

        self
    }

    pub fn with_client_protocols(mut self, protocols: HashSet<String>) -> Self {
        self.client = protocols;

        self
    }
}

/// The request id used to identify a gRPC request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestId(pub(crate) u64);

impl Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

type ChannelNegotiationFuture =
    BoxFuture<'static, (Result<Channel, tonic::transport::Error>, RequestId, PeerId)>;

/// gRPC behaviour for libp2p
///
/// That allows to open gRPC connections to peers and to accept incoming gRPC connections.
/// It also handles the negotiation of the gRPC channel. Once the channel is established,
/// the behaviour will return a [`GrpcStream`] that can be used to send and receive gRPC messages.
/// A gRPC Router is optional because as a client or light client I need be able to open a connection
/// to a peer without having a gRPC service to expose.
pub(crate) struct Behaviour {
    /// The optional gRPC service to expose
    service: Option<Router>,
    /// The next request id to use
    next_request_id: RequestId,
    /// The next inbound request id to use
    next_inbound_request_id: Arc<AtomicU64>,
    /// The list of connected peers with the associated gRPC channel
    connected: HashMap<PeerId, SmallVec<[Connection; 2]>>,
    /// The list of known addresses for each peer managed by `add_address` and `remove_address`
    addresses: HashMap<PeerId, SmallVec<[Multiaddr; 6]>>,
    /// The optional inbound stream to receive gRPC connections
    inbound_stream: Option<mpsc::UnboundedSender<io::Result<stream::GrpcStream>>>,
    /// The list of pending outbound connections
    pending_outbound_connections: HashMap<PeerId, OutboundConnectionRequest>,
    /// The list of pending events to send to the swarm
    pending_events: VecDeque<ToSwarm<Event, ProtocolRequest>>,
    /// The list of pending channel negotiation futures
    pending_negotiated_channels: FuturesUnordered<ChannelNegotiationFuture>,
    inbound_protocols: HashSet<String>,
    outbound_protocols: HashSet<String>,
}

impl Behaviour {
    // TODO: Remove unused when gRPC behaviour is activated
    pub fn new(service: GrpcContext) -> Self {
        let (service, (inbound_protocols, outbound_protocols)) = service.into_parts();

        Self {
            service,
            inbound_protocols,
            outbound_protocols,
            connected: HashMap::new(),
            addresses: HashMap::new(),
            inbound_stream: None,
            next_request_id: RequestId(1),
            next_inbound_request_id: Arc::new(AtomicU64::new(0)),
            pending_outbound_connections: HashMap::new(),
            pending_events: VecDeque::new(),
            pending_negotiated_channels: FuturesUnordered::new(),
        }
    }

    /// Adds a known address for a peer that can be used for
    /// dialing attempts by the `Swarm`
    ///
    /// Addresses added in this way are only removed by `remove_address`.
    #[cfg(test)]
    pub fn add_address(&mut self, peer: &PeerId, address: Multiaddr) {
        self.addresses.entry(*peer).or_default().push(address);
    }

    /// Removes an address of a peer previously added via `add_address`.
    #[cfg(test)]
    #[allow(unused)]
    pub fn remove_address(&mut self, peer: &PeerId, address: &Multiaddr) {
        let mut last = false;
        if let Some(addresses) = self.addresses.get_mut(peer) {
            addresses.retain(|a| a != address);
            last = addresses.is_empty();
        }
        if last {
            self.addresses.remove(peer);
        }
    }

    /// Ask the behaviour to create a new outbound connection for the given peer.
    ///
    /// The return value is a [`OutboundConnection`] that can be used to check the status of the
    /// connection. If the connection is pending, the request id is returned. If the connection
    /// is established, the gRPC channel is returned.
    // TODO: Remove unused when gRPC behaviour is activated
    #[allow(unused)]
    pub fn open_outbound_connection(
        &mut self,
        peer_id: &PeerId,
        protocol: String,
    ) -> OutboundConnection {
        // If there is a pending outbound connection for this peer
        // return the request id
        if let Some(request) = self.pending_outbound_connections.get(peer_id) {
            return OutboundConnection::Pending {
                request_id: request.request_id,
            };
        }

        if let Some(connections) = self.connected.get_mut(peer_id) {
            match connections.first() {
                Some(Connection {
                    id,
                    address,
                    request_id: Some(request_id),
                    channel: Some(channel),
                }) => OutboundConnection::Connected(OutboundConnectedConnection {
                    request_id: *request_id,
                    channel: channel.clone(),
                }),
                Some(Connection {
                    id,
                    address,
                    request_id: Some(request_id),
                    channel,
                }) => {
                    debug!("Peer already connected but no channel bound");

                    OutboundConnection::Pending {
                        request_id: *request_id,
                    }
                }
                Some(_) => self.open_connection(peer_id, protocol),
                _ => {
                    debug!("No connection for this peer {}", peer_id);
                    self.open_connection(peer_id, protocol)
                }
            }
        } else {
            debug!("Buffering sender as no available connection to peer {peer_id} yet");
            self.open_connection(peer_id, protocol)
        }
    }

    /// Return the next outbound request id
    fn next_request_id(&mut self) -> RequestId {
        let request_id = self.next_request_id;
        self.next_request_id.0 += 1;

        request_id
    }

    /// Try to open a connection with the given peer.
    fn open_connection(&mut self, peer_id: &PeerId, protocol: String) -> OutboundConnection {
        info!("Opening gRPC outbound connection to peer {peer_id}");

        let (notifier, receiver) = oneshot::channel();
        let request_id = self.next_request_id();

        self.pending_outbound_connections
            .entry(*peer_id)
            .or_insert_with(|| OutboundConnectionRequest {
                request_id,
                notifier,
                protocol,
            });

        self.pending_events.push_back(ToSwarm::Dial {
            opts: DialOpts::peer_id(*peer_id).build(),
        });

        OutboundConnection::Opening {
            request_id,
            receiver,
        }
    }

    /// Handle the [`ConnectionEstablished`] event coming from the [`Swarm`]
    /// and try to open a gRPC channel using a [`ConnectionHandler`].
    fn on_connection_established(
        &mut self,
        ConnectionEstablished {
            peer_id,
            connection_id,
            endpoint,
            failed_addresses,
            other_established,
        }: ConnectionEstablished,
    ) {
        let address = match endpoint {
            ConnectedPoint::Dialer { address, .. } => Some(address.clone()),
            ConnectedPoint::Listener { .. } => None,
        };

        let connection = Connection {
            id: connection_id,
            address,
            request_id: None,
            channel: None,
        };

        self.connected.entry(peer_id).or_default().push(connection);

        // TODO refactor this to better handle connection
        // If there is no current established connection it means that it's the
        // first connection with that peer
        if other_established == 0 {
            self.try_connect(&peer_id);
        }
    }

    /// Starts the gRPC service if not already started
    fn on_new_listener(&mut self, listener_id: ListenerId) {
        if let Some(service) = self.service.take() {
            let (tx, rx) = mpsc::unbounded_channel();
            self.inbound_stream = Some(tx);
            // TODO: TP-758: Switch to serve_with_incoming_shutdown at some point
            tokio::spawn(service.serve_with_incoming(proxy::GrpcProxy::new(rx)));
            info!("New gRPC proxy started and listening on {listener_id:?}");
        } else {
            warn!(
                "Tried to instantiate a gRPC proxy on {listener_id:?} but the service is missing \
                 (already spawn or unprovided)"
            );
        }
    }

    /// On [`ConnectionClosed`] we cleanup the `connected` state of the behaviour.
    fn on_connection_closed(
        &mut self,
        ConnectionClosed {
            peer_id,
            connection_id,
            endpoint,
            handler,
            remaining_established,
        }: ConnectionClosed<<Self as NetworkBehaviour>::ConnectionHandler>,
    ) {
        debug!("Connection {connection_id} closed with peer {peer_id}");
        if let Some(connections) = self.connected.get_mut(&peer_id) {
            connections.retain(|conn| conn.id != connection_id);
            if connections.is_empty() {
                self.connected.remove(&peer_id);
            }
        }
    }

    /// Handle the [`DialFailure`] event comming from the [`Swarm`]
    fn on_dial_failure(
        &mut self,
        DialFailure {
            peer_id,
            error,
            connection_id,
        }: DialFailure,
    ) {
        if let Some(peer_id) = peer_id {
            match error {
                DialError::DialPeerConditionFalse(_) => {
                    self.try_connect(&peer_id);
                }
                _ => {
                    if let Some(OutboundConnectionRequest {
                        request_id,
                        notifier,
                        protocol,
                    }) = self.pending_outbound_connections.remove(&peer_id)
                    {
                        self.pending_events.push_back(ToSwarm::GenerateEvent(
                            Event::OutboundFailure {
                                peer_id,
                                request_id,
                                error: OutboundError::DialFailure,
                            },
                        ));

                        let _ = notifier.send(Err(OutboundError::DialFailure));
                    }
                }
            }
        }
    }

    /// Try to connect an opened outbound connection with a [`ConnectionHandler`]
    /// in order to handle the request.
    fn try_connect(&mut self, peer_id: &PeerId) {
        if let Some(connections) = self.connected.get_mut(peer_id) {
            let connection = connections.first_mut();
            if let Some(connection) = connection {
                if let Some(OutboundConnectionRequest {
                    request_id,
                    notifier,
                    protocol,
                }) = self.pending_outbound_connections.get(peer_id)
                {
                    debug!("gRPC Outbound connection established with {peer_id}");
                    self.pending_events.push_back(ToSwarm::NotifyHandler {
                        peer_id: *peer_id,
                        handler: libp2p::swarm::NotifyHandler::One(connection.id),
                        event: ProtocolRequest {
                            request_id: *request_id,
                            protocol: protocol.clone(),
                        },
                    });

                    debug!("Sending handler notif to connect");
                }
            }
        }
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = Handler;

    type ToSwarm = Event;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        local_addr: &libp2p::Multiaddr,
        remote_addr: &libp2p::Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(Handler::new(
            self.next_inbound_request_id.clone(),
            self.inbound_protocols.clone(),
        ))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        addr: &libp2p::Multiaddr,
        role_override: libp2p::core::Endpoint,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(Handler::new(
            self.next_inbound_request_id.clone(),
            self.outbound_protocols.clone(),
        ))
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _connection_id: libp2p::swarm::ConnectionId,
        maybe_peer: Option<PeerId>,
        _addresses: &[libp2p::Multiaddr],
        _effective_role: libp2p::core::Endpoint,
    ) -> Result<Vec<libp2p::Multiaddr>, libp2p::swarm::ConnectionDenied> {
        let peer_id = match maybe_peer {
            None => return Ok(vec![]),
            Some(peer_id) => peer_id,
        };

        let mut addresses = Vec::new();
        if let Some(connections) = self.connected.get(&peer_id) {
            addresses.extend(connections.iter().filter_map(|c| c.address.clone()));
        }

        if let Some(more) = self.addresses.get(&peer_id) {
            addresses.extend(more.into_iter().cloned());
        }

        Ok(addresses)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: libp2p::swarm::ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        match event {
            handler::event::Event::OutboundTimeout(request) => {
                debug!(
                    "Outbound timeout for request {} with peer {peer_id}",
                    request.request_id
                );
                self.pending_events
                    .push_back(ToSwarm::GenerateEvent(Event::OutboundFailure {
                        peer_id,
                        request_id: request.request_id,
                        error: OutboundError::Timeout,
                    }));

                if let Some(connection_request) = self.pending_outbound_connections.remove(&peer_id)
                {
                    _ = connection_request
                        .notifier
                        .send(Err(OutboundError::Timeout))
                }
            }
            handler::event::Event::UnsupportedProtocol(request_id, protocol) => {
                debug!(
                    "Unsupported protocol {protocol} for request {request_id} with peer {peer_id}"
                );
                self.pending_events
                    .push_back(ToSwarm::GenerateEvent(Event::OutboundFailure {
                        peer_id,
                        request_id,
                        error: OutboundError::UnsupportedProtocol(protocol.clone()),
                    }));

                if let Some(connection_request) = self.pending_outbound_connections.remove(&peer_id)
                {
                    _ = connection_request
                        .notifier
                        .send(Err(OutboundError::UnsupportedProtocol(protocol)))
                }
            }
            handler::event::Event::InboundNegotiatedStream { request_id, stream } => {
                debug!("Inbound stream negotiated for request {request_id} with peer {peer_id}",);
                if let Some(sender) = &mut self.inbound_stream {
                    _ = sender.send(Ok(GrpcStream::new(stream, peer_id, connection_id)));
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        Event::InboundNegotiatedConnection {
                            request_id,
                            connection_id,
                        },
                    ));
                }
            }
            handler::event::Event::OutboundNegotiatedStream { request_id, stream } => {
                debug!("Outbound stream negotiated for request {request_id} with peer {peer_id}",);
                let stream = GrpcStream::new(stream, peer_id, connection_id);

                let future = stream
                    .into_channel()
                    .map(move |channel| (channel, request_id, peer_id))
                    .boxed();

                self.pending_negotiated_channels.push(future);
            }
        }
    }

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionEstablished(connection_established) => {
                self.on_connection_established(connection_established)
            }
            FromSwarm::NewListener(NewListener { listener_id }) => {
                self.on_new_listener(listener_id)
            }
            FromSwarm::ConnectionClosed(connection_closed) => {
                self.on_connection_closed(connection_closed)
            }
            FromSwarm::DialFailure(dial_failure) => self.on_dial_failure(dial_failure),
            FromSwarm::AddressChange(_)
            | FromSwarm::ExpiredListenAddr(_)
            | FromSwarm::ExternalAddrConfirmed(_)
            | FromSwarm::ExternalAddrExpired(_)
            | FromSwarm::ListenFailure(_)
            | FromSwarm::ListenerClosed(_)
            | FromSwarm::ListenerError(_)
            | FromSwarm::NewExternalAddrCandidate(_)
            | FromSwarm::NewListenAddr(_) => (),
        }
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        params: &mut impl libp2p::swarm::PollParameters,
    ) -> Poll<libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        // Sending event to both `Swarm` and `ConnectionHandler`
        if let Some(ev) = self.pending_events.pop_front() {
            return Poll::Ready(ev);
        }

        // When channel has been negotiated by the [`ConnectionHandler`] we need
        // to update the [`Connection`] with the channel.
        match self.pending_negotiated_channels.poll_next_unpin(cx) {
            Poll::Ready(Some((Ok(channel), request_id, peer_id))) => {
                debug!("gRPC channel ready for {} {}", peer_id, request_id);
                if let Some(conns) = self.connected.get_mut(&peer_id) {
                    for conn in conns {
                        if let Some(conn_request_id) = &conn.request_id {
                            if request_id == *conn_request_id {
                                conn.channel = Some(channel.clone());

                                break;
                            }
                        }
                    }
                }

                // Notifying the channel to the initial sender
                if let Some(req) = self.pending_outbound_connections.remove(&peer_id) {
                    let _ = req.notifier.send(Ok(channel.clone()));
                    self.pending_events.push_back(ToSwarm::GenerateEvent(
                        Event::OutboundNegotiatedConnection {
                            request_id: req.request_id,
                            peer_id,
                        },
                    ));
                }

                return Poll::Ready(ToSwarm::GenerateEvent(Event::OutboundSuccess {
                    peer_id,
                    request_id,
                    channel,
                }));
            }

            Poll::Ready(Some((Err(error), request_id, peer_id))) => {
                debug!("Received error from channel negotiation {:?}", error);
                let error = Arc::new(error);
                if let Some(req) = self.pending_outbound_connections.remove(&peer_id) {
                    let _ = req
                        .notifier
                        .send(Err(OutboundError::GrpcChannel(error.clone())));
                }

                return Poll::Ready(ToSwarm::GenerateEvent(Event::OutboundFailure {
                    peer_id,
                    request_id,
                    error: OutboundError::GrpcChannel(error),
                }));
            }
            _ => {}
        }

        Poll::Pending
    }
}
