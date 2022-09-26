use self::{
    codec::{TransmissionCodec, TransmissionRequest, TransmissionResponse},
    protocol::TransmissionProtocol,
};

use libp2p::{
    core::{connection::ConnectionId, ConnectedPoint},
    request_response::{
        handler::{RequestResponseHandler, RequestResponseHandlerEvent},
        ProtocolSupport, RequestId, RequestResponse, RequestResponseConfig, RequestResponseEvent,
        RequestResponseMessage, ResponseChannel,
    },
    swarm::{DialError, NetworkBehaviour, NetworkBehaviourAction, PollParameters},
    Multiaddr, PeerId,
};
use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    iter,
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::oneshot::{self, Sender};
use tracing::{debug, error};

pub mod codec;
pub mod protocol;

type PendingRequests = HashMap<RequestId, oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>>;

/// Transmission is responsible of dealing with node interaction (RequestResponse, Gossip)
pub(crate) struct TransmissionBehaviour {
    pub inner: RequestResponse<TransmissionCodec>,

    pending_requests: PendingRequests,
    events: VecDeque<TransmissionOut>,
}

#[derive(Debug)]
pub enum TransmissionOut {
    Request {
        from: PeerId,
        data: Vec<u8>,
        channel: ResponseChannel<TransmissionResponse>,
    },
}

impl NetworkBehaviour for TransmissionBehaviour {
    type ConnectionHandler = RequestResponseHandler<TransmissionCodec>;

    type OutEvent = TransmissionOut;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        self.inner.new_handler()
    }

    fn addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
        self.inner.addresses_of_peer(peer_id)
    }

    fn inject_connection_established(
        &mut self,
        peer_id: &PeerId,
        connection_id: &ConnectionId,
        endpoint: &ConnectedPoint,
        failed_addresses: Option<&Vec<Multiaddr>>,
        other_established: usize,
    ) {
        debug!("Connection established with peer: {peer_id}");
        self.inner.inject_connection_established(
            peer_id,
            connection_id,
            endpoint,
            failed_addresses,
            other_established,
        )
    }

    fn inject_connection_closed(
        &mut self,
        peer_id: &PeerId,
        connection_id: &ConnectionId,
        connected_point: &ConnectedPoint,
        handler: RequestResponseHandler<TransmissionCodec>,
        remaining_established: usize,
    ) {
        debug!("Connection closed for peer: {peer_id}");
        self.inner.inject_connection_closed(
            peer_id,
            connection_id,
            connected_point,
            handler,
            remaining_established,
        )
    }

    fn inject_address_change(
        &mut self,
        peer_id: &PeerId,
        conn: &ConnectionId,
        old: &ConnectedPoint,
        new: &ConnectedPoint,
    ) {
        self.inner.inject_address_change(peer_id, conn, old, new)
    }

    fn inject_event(
        &mut self,
        peer_id: PeerId,
        connection: ConnectionId,
        event: RequestResponseHandlerEvent<TransmissionCodec>,
    ) {
        self.inner.inject_event(peer_id, connection, event);
    }

    fn inject_dial_failure(
        &mut self,
        peer_id: Option<PeerId>,
        handler: Self::ConnectionHandler,
        error: &DialError,
    ) {
        error!("Dial failure: {error:?}");

        self.inner.inject_dial_failure(peer_id, handler, error);
    }

    fn inject_listen_failure(
        &mut self,
        local_addr: &Multiaddr,
        send_back_addr: &Multiaddr,
        handler: Self::ConnectionHandler,
    ) {
        self.inner
            .inject_listen_failure(local_addr, send_back_addr, handler)
    }

    fn inject_new_listener(&mut self, id: libp2p::core::transport::ListenerId) {
        self.inner.inject_new_listener(id)
    }

    fn inject_new_listen_addr(
        &mut self,
        id: libp2p::core::transport::ListenerId,
        addr: &Multiaddr,
    ) {
        self.inner.inject_new_listen_addr(id, addr)
    }

    fn inject_expired_listen_addr(
        &mut self,
        id: libp2p::core::transport::ListenerId,
        addr: &Multiaddr,
    ) {
        self.inner.inject_expired_listen_addr(id, addr);
    }

    fn inject_listener_error(
        &mut self,
        id: libp2p::core::transport::ListenerId,
        err: &(dyn std::error::Error + 'static),
    ) {
        self.inner.inject_listener_error(id, err);
    }

    fn inject_listener_closed(
        &mut self,
        id: libp2p::core::transport::ListenerId,
        reason: Result<(), &std::io::Error>,
    ) {
        self.inner.inject_listener_closed(id, reason);
    }

    fn inject_new_external_addr(&mut self, addr: &Multiaddr) {
        self.inner.inject_new_external_addr(addr)
    }

    fn inject_expired_external_addr(&mut self, addr: &Multiaddr) {
        self.inner.inject_expired_external_addr(addr)
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(event));
        }

        while let Poll::Ready(req_action) = self.inner.poll(cx, params) {
            match req_action {
                NetworkBehaviourAction::GenerateEvent(event) => match event {
                    RequestResponseEvent::Message {
                        peer,
                        message:
                            RequestResponseMessage::Request {
                                request, channel, ..
                            },
                    } => {
                        return Poll::Ready(NetworkBehaviourAction::GenerateEvent(
                            TransmissionOut::Request {
                                channel,
                                from: peer,
                                data: request.0,
                            },
                        ));
                    }

                    RequestResponseEvent::Message {
                        message:
                            RequestResponseMessage::Response {
                                request_id,
                                response,
                            },
                        ..
                    } => {
                        if let Some(sender) = self.pending_requests.remove(&request_id) {
                            let _ = sender.send(Ok(response.0));
                        }
                    }

                    RequestResponseEvent::OutboundFailure {
                        request_id, error, ..
                    } => {
                        let _ = self
                            .pending_requests
                            .remove(&request_id)
                            .expect("Request to still be pending.")
                            .send(Err(Box::new(error)));
                    }

                    RequestResponseEvent::InboundFailure {
                        error, request_id, ..
                    } => {
                        error!("InboundFailure {error:?} {request_id}")
                    }

                    RequestResponseEvent::ResponseSent { .. } => {}
                },

                NetworkBehaviourAction::Dial { opts, handler } => {
                    if opts.get_peer_id().is_none() {
                        error!("The request-response isn't supposed to start dialing addresses");
                    }
                    return Poll::Ready(NetworkBehaviourAction::Dial { opts, handler });
                }
                NetworkBehaviourAction::NotifyHandler {
                    peer_id,
                    handler,
                    event,
                } => {
                    return Poll::Ready(NetworkBehaviourAction::NotifyHandler {
                        peer_id,
                        handler,
                        event,
                    });
                }
                NetworkBehaviourAction::ReportObservedAddr { .. } => todo!(),
                NetworkBehaviourAction::CloseConnection { .. } => todo!(),
            }
        }

        Poll::Pending
    }
}

impl TransmissionBehaviour {
    pub fn new() -> Self {
        let mut cfg = RequestResponseConfig::default();
        cfg.set_connection_keep_alive(Duration::from_secs(10));
        cfg.set_request_timeout(Duration::from_secs(30));

        Self {
            inner: RequestResponse::new(
                TransmissionCodec(),
                iter::once((TransmissionProtocol(), ProtocolSupport::Full)),
                cfg,
            ),
            pending_requests: HashMap::new(),
            events: VecDeque::new(),
        }
    }

    pub(crate) fn send_request(
        &mut self,
        to: &PeerId,
        data: TransmissionRequest,
        sender: Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
    ) -> Result<(), ()> {
        let request_id = self.inner.send_request(to, data);

        self.pending_requests.insert(request_id, sender);

        Ok(())
    }

    pub(crate) fn send_response(
        &mut self,
        channel: ResponseChannel<TransmissionResponse>,
        data: TransmissionResponse,
    ) -> Result<(), TransmissionResponse> {
        self.inner.send_response(channel, data)
    }
}
