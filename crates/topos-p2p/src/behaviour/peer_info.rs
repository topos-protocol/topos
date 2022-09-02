use std::{
    collections::VecDeque,
    task::{Context, Poll},
};

use libp2p::{
    core::{connection::ConnectionId, ConnectedPoint},
    identify::{Identify, IdentifyConfig, IdentifyEvent, IdentifyInfo},
    identity::Keypair,
    swarm::{
        ConnectionHandler, IntoConnectionHandler, NetworkBehaviour, NetworkBehaviourAction,
        PollParameters,
    },
    Multiaddr, PeerId,
};
use tracing::{error, info};

pub struct PeerInfoBehaviour {
    identify: Identify,

    /// Events to dispatch in the stream
    events: VecDeque<PeerInfoOut>,
}

/// Event that can be emitted by the behaviour.
#[derive(Debug)]
pub enum PeerInfoOut {
    /// We have obtained identity information from a peer, including the addresses it is listening
    /// on.
    Identified {
        /// Id of the peer that has been identified.
        peer_id: PeerId,
        /// Information about the peer.
        info: Box<IdentifyInfo>,
    },

    Disconnected {
        peer_id: PeerId,
    },
}

impl NetworkBehaviour for PeerInfoBehaviour {
    type ConnectionHandler = <Identify as NetworkBehaviour>::ConnectionHandler;

    type OutEvent = PeerInfoOut;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        self.identify.new_handler()
    }

    fn inject_connection_established(
        &mut self,
        peer_id: &PeerId,
        connection_id: &ConnectionId,
        endpoint: &ConnectedPoint,
        failed_addresses: Option<&Vec<Multiaddr>>,
        other_established: usize,
    ) {
        self.identify.inject_connection_established(
            peer_id,
            connection_id,
            endpoint,
            failed_addresses,
            other_established,
        );
    }
    fn inject_event(
        &mut self,
        peer_id: PeerId,
        connection: ConnectionId,
        event: <<Self::ConnectionHandler as IntoConnectionHandler>::Handler as ConnectionHandler>::OutEvent,
    ) {
        self.identify.inject_event(peer_id, connection, event)
    }

    fn inject_connection_closed(
        &mut self,
        peer_id: &PeerId,
        _: &ConnectionId,
        _: &ConnectedPoint,
        _: <Self::ConnectionHandler as IntoConnectionHandler>::Handler,
        remaining_established: usize,
    ) {
        if remaining_established == 0 {
            self.events
                .push_back(PeerInfoOut::Disconnected { peer_id: *peer_id })
        }
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(event));
        }

        while let Poll::Ready(action) = self.identify.poll(cx, params) {
            match action {
                NetworkBehaviourAction::GenerateEvent(event) => match event {
                    IdentifyEvent::Received { peer_id, info, .. } => {
                        return Poll::Ready(NetworkBehaviourAction::GenerateEvent(
                            PeerInfoOut::Identified {
                                peer_id,
                                info: Box::new(info),
                            },
                        ));
                    }
                    IdentifyEvent::Error { peer_id, error } => {
                        info!("Identification with peer {:?} failed => {}", peer_id, error)
                    }
                    IdentifyEvent::Pushed { .. } => {}
                    IdentifyEvent::Sent { .. } => {}
                },
                NetworkBehaviourAction::Dial { opts, handler } => {
                    if opts.get_peer_id().is_none() {
                        error!("The peer-info isn't supposed to start dialing addresses");
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
                    })
                }
                NetworkBehaviourAction::ReportObservedAddr { address, score } => {
                    return Poll::Ready(NetworkBehaviourAction::ReportObservedAddr {
                        address,
                        score,
                    })
                }
                NetworkBehaviourAction::CloseConnection {
                    peer_id,
                    connection,
                } => {
                    return Poll::Ready(NetworkBehaviourAction::CloseConnection {
                        peer_id,
                        connection,
                    });
                }
            }
        }

        Poll::Pending
    }
}

impl PeerInfoBehaviour {
    pub(crate) fn new(identify_protocol: &'static str, peer_key: &Keypair) -> PeerInfoBehaviour {
        let ident_config = IdentifyConfig::new(identify_protocol.to_string(), peer_key.public())
            .with_push_listen_addr_updates(true);
        let identify = Identify::new(ident_config);

        Self {
            identify,
            events: VecDeque::new(),
        }
    }
}
