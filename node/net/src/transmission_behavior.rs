//! Behavior for data transmission in request-response fashion
//! as needed for Topos Reliable Broadcast
//!
//! Design note - dependency from here to lib's NetworkEvents and NetworkCommands
//! is intentional.
//!
use crate::{
    upgrade::{read_length_prefixed, write_length_prefixed},
    NetworkCommands, NetworkEvents, PeerId,
};
use async_trait::async_trait;
use libp2p::{
    futures::{AsyncRead, AsyncWrite, AsyncWriteExt},
    request_response::{
        ProtocolName, ProtocolSupport, RequestResponse, RequestResponseCodec, RequestResponseEvent,
    },
    request_response::{RequestId, RequestResponseMessage, ResponseChannel},
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour,
};
use std::{collections::HashSet, iter};
use tokio::{
    sync::mpsc::error::SendError,
    sync::oneshot::error::RecvError,
    sync::{mpsc, oneshot},
};

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
pub(crate) struct TransmissionBehavior {
    pub rr: RequestResponse<TransmissionCodec>,
    #[behaviour(ignore)]
    pub tx_events: mpsc::UnboundedSender<NetworkEvents>,
    #[behaviour(ignore)]
    req_ids_to_ext_ids: HashSet<RequestId>,
}

impl TransmissionBehavior {
    /// Factory
    pub(crate) fn new(events_sender: mpsc::UnboundedSender<NetworkEvents>) -> Self {
        Self {
            rr: RequestResponse::new(
                TransmissionCodec(),
                iter::once((TransmissionProtocol(), ProtocolSupport::Full)),
                Default::default(),
            ),
            tx_events: events_sender,
            req_ids_to_ext_ids: HashSet::new(),
        }
    }

    /// Executes command
    pub(crate) fn eval(&mut self, cmd: NetworkCommands) {
        match cmd {
            NetworkCommands::TransmissionReq { to, data } => {
                for peer_id in to {
                    //  publish
                    let req_id = self
                        .rr
                        .send_request(&peer_id, TransmissionRequest(data.clone()));
                    //  remember each (req_id)
                    self.req_ids_to_ext_ids.insert(req_id);
                }
            }
        }
    }

    /// Called by handler of inbound request message
    async fn on_inbound_request(
        &mut self,
        peer: PeerId,
        _req_id: RequestId,
        req_payload: TransmissionRequest,
        resp_chan: ResponseChannel<TransmissionResponse>,
    ) -> Result<(), TransmissionInternalErr> {
        self.tx_events.send(NetworkEvents::TransmissionOnReq {
            from: peer,
            data: req_payload.0,
        })?;
        //  send the response back, error handled in other event handling branches
        self.rr
            .send_response(resp_chan, TransmissionResponse(vec![]))?;
        Ok(())
    }

    /// Called by handler of inbound response message
    async fn on_inbound_response(
        &mut self,
        _peer: PeerId,
        request_id: RequestId,
        _response: TransmissionResponse,
    ) -> Result<(), TransmissionInternalErr> {
        if self.req_ids_to_ext_ids.remove(&request_id) {
            return Err(TransmissionInternalErr::ReqNotFound(request_id));
        }
        Ok(())
    }
}

impl NetworkBehaviourEventProcess<RequestResponseEvent<TransmissionRequest, TransmissionResponse>>
    for TransmissionBehavior
{
    fn inject_event(
        &mut self,
        event: RequestResponseEvent<TransmissionRequest, TransmissionResponse>,
    ) {
        match event {
            RequestResponseEvent::Message { peer, message } => match message {
                RequestResponseMessage::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    let _ = self.on_inbound_request(peer, request_id, request, channel);
                }
                RequestResponseMessage::Response {
                    request_id,
                    response,
                } => {
                    let _ = self.on_inbound_response(peer, request_id, response);
                }
            },
            RequestResponseEvent::OutboundFailure {
                peer,
                request_id: _request_id,
                error,
            } => {
                log::warn!("Outbound failure - peer:{:?}, err: {:?}", peer, error);
            }
            RequestResponseEvent::InboundFailure {
                peer,
                request_id: _request_id,
                error,
            } => {
                log::warn!("Inbound failure - peer: {:?}, err: {:?}", peer, error);
            }
            RequestResponseEvent::ResponseSent { .. } => {}
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TransmissionProtocol();

#[derive(Clone)]
pub(crate) struct TransmissionCodec();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransmissionRequest(Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransmissionResponse(Vec<u8>);

impl ProtocolName for TransmissionProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/trbp-transmission/1".as_bytes()
    }
}

#[async_trait]
impl RequestResponseCodec for TransmissionCodec {
    type Protocol = TransmissionProtocol;
    type Request = TransmissionRequest;
    type Response = TransmissionResponse;

    async fn read_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1_000_000_000).await?;
        Ok(TransmissionRequest(vec))
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1_000_000_000).await?;
        Ok(TransmissionResponse(vec))
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, req.0).await?;
        io.close().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, res.0).await?;
        io.close().await?;
        Ok(())
    }
}

enum TransmissionInternalErr {
    MpscSendFailed(mpsc::error::SendError<NetworkEvents>),
    OneshotRecvFailed(oneshot::error::RecvError),
    SendRespFailed(TransmissionResponse),
    ReqNotFound(RequestId),
}

impl From<mpsc::error::SendError<NetworkEvents>> for TransmissionInternalErr {
    fn from(err: SendError<NetworkEvents>) -> Self {
        TransmissionInternalErr::MpscSendFailed(err)
    }
}

impl From<oneshot::error::RecvError> for TransmissionInternalErr {
    fn from(err: RecvError) -> Self {
        TransmissionInternalErr::OneshotRecvFailed(err)
    }
}

impl From<TransmissionResponse> for TransmissionInternalErr {
    fn from(err: TransmissionResponse) -> Self {
        TransmissionInternalErr::SendRespFailed(err)
    }
}
