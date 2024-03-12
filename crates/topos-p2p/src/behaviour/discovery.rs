use std::borrow::Cow;
use std::pin::Pin;
use std::task::Poll;

use crate::error::P2PError;
use crate::{config::DiscoveryConfig, error::CommandExecutionError};

use libp2p::kad::{
    BootstrapOk, BootstrapResult, Event as KademliaEvent, ProgressStep, QueryId, QueryResult,
};
use libp2p::swarm::ToSwarm;
use libp2p::{
    identity::Keypair,
    kad::{store::MemoryStore, Behaviour, BucketInserts, Config},
    swarm::NetworkBehaviour,
    Multiaddr, PeerId,
};
use tokio::sync::oneshot;
use tracing::{debug, error, info};

pub type PendingRecordRequest = oneshot::Sender<Result<Vec<Multiaddr>, CommandExecutionError>>;

/// DiscoveryBehaviour is responsible to discover and manage connections with peers
pub(crate) struct DiscoveryBehaviour {
    /// The inner kademlia behaviour
    pub(crate) inner: Behaviour<MemoryStore>,
    /// The current bootstrap query id used to track the progress of the bootstrap
    /// and to avoid to start a new bootstrap query if the previous one is still in progress
    pub(crate) current_bootstrap_query_id: Option<QueryId>,
    /// The next bootstrap query interval used to schedule the next bootstrap query
    pub(crate) next_bootstrap_query: Option<Pin<Box<tokio::time::Interval>>>,
}

impl DiscoveryBehaviour {
    pub fn create(
        config: &DiscoveryConfig,
        peer_key: Keypair,
        discovery_protocol: Cow<'static, [u8]>,
        known_peers: &[(PeerId, Multiaddr)],
        _with_mdns: bool,
    ) -> Self {
        let local_peer_id = peer_key.public().to_peer_id();
        let kademlia_config = Config::default()
            .set_replication_factor(config.replication_factor)
            .set_kbucket_inserts(BucketInserts::Manual)
            .set_replication_interval(config.replication_interval)
            .set_publication_interval(config.publication_interval)
            .set_provider_publication_interval(config.provider_publication_interval)
            .to_owned();

        let mut kademlia = Behaviour::with_config(
            local_peer_id,
            MemoryStore::new(local_peer_id),
            kademlia_config,
        );

        for known_peer in known_peers {
            info!(
                "Adding the known peer:{} reachable at {}",
                &known_peer.0, &known_peer.1
            );
            let x = kademlia.add_address(&known_peer.0, known_peer.1.clone());
            info!(
                "Adding the known peer:{} reachable at {} - {:?}",
                &known_peer.0, &known_peer.1, x
            );
        }

        Self {
            inner: kademlia,
            current_bootstrap_query_id: None,
            next_bootstrap_query: Some(Box::pin(tokio::time::interval(config.bootstrap_interval))),
        }
    }

    /// Start the kademlia bootstrap process if it is not already in progress.
    /// The bootstrap process is used to discover new peers in the network.
    /// The bootstrap process starts by sending a `FIND_NODE` query of the local PeerId in the DHT.
    /// Then multiple random PeerId are created in order to randomly walk the network.
    pub fn bootstrap(&mut self) -> Result<(), P2PError> {
        if self.current_bootstrap_query_id.is_none() {
            match self.inner.bootstrap() {
                Ok(query_id) => {
                    info!("Started kademlia bootstrap with query_id: {query_id:?}");
                    self.current_bootstrap_query_id = Some(query_id);
                }
                Err(error) => {
                    error!("Unable to start kademlia bootstrap: {error:?}");
                    return Err(P2PError::BootstrapError(
                        "Unable to start kademlia bootstrap",
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn get_addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
        if let Some(key_ref) = self.inner.kbucket(*peer_id) {
            key_ref
                .iter()
                .filter(|e| e.node.key.preimage() == peer_id)
                .map(|e| e.node.value.first().clone())
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl NetworkBehaviour for DiscoveryBehaviour {
    type ConnectionHandler = <Behaviour<MemoryStore> as NetworkBehaviour>::ConnectionHandler;

    type ToSwarm = KademliaEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.inner.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: libp2p::core::Endpoint,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.inner
            .handle_established_outbound_connection(connection_id, peer, addr, role_override)
    }

    fn on_swarm_event(&mut self, event: libp2p::swarm::FromSwarm) {
        self.inner.on_swarm_event(event)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: libp2p::swarm::ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        self.inner
            .on_connection_handler_event(peer_id, connection_id, event)
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<libp2p::swarm::ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        // Poll the kademlia bootstrap interval future in order to define if we need to call the
        // `bootstrap`
        if let Some(next_bootstrap_query) = self.next_bootstrap_query.as_mut() {
            if next_bootstrap_query.poll_tick(cx).is_ready() {
                if let Err(error) = self.bootstrap() {
                    error!("Error while create bootstrap query: {error:?}");
                }
            }
        }

        if let Poll::Ready(event) = self.inner.poll(cx) {
            match event {
                // When a Bootstrap query ends, we reset the `query_id`
                ToSwarm::GenerateEvent(KademliaEvent::OutboundQueryProgressed {
                    id,
                    result:
                        result @ QueryResult::Bootstrap(BootstrapResult::Ok(BootstrapOk {
                            num_remaining: 0,
                            ..
                        })),
                    step: step @ ProgressStep { last: true, .. },
                    stats,
                }) if Some(&id) == self.current_bootstrap_query_id.as_ref() => {
                    if let Some(interval) = self.next_bootstrap_query.as_mut() {
                        interval.reset();
                    };

                    self.current_bootstrap_query_id = None;
                    debug!("Kademlia bootstrap completed with query_id: {id:?}");

                    return Poll::Ready(ToSwarm::GenerateEvent(
                        KademliaEvent::OutboundQueryProgressed {
                            id,
                            result,
                            stats,
                            step,
                        },
                    ));
                }
                event => {
                    return Poll::Ready(event);
                }
            }
        }

        Poll::Pending
    }

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), libp2p::swarm::ConnectionDenied> {
        self.inner
            .handle_pending_inbound_connection(connection_id, local_addr, remote_addr)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        maybe_peer: Option<PeerId>,
        addresses: &[Multiaddr],
        effective_role: libp2p::core::Endpoint,
    ) -> Result<Vec<Multiaddr>, libp2p::swarm::ConnectionDenied> {
        self.inner.handle_pending_outbound_connection(
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )
    }
}
