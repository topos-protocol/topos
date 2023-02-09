use std::collections::{HashMap, HashSet};

use crate::{
    behaviour::{
        discovery::{PendingDials, PendingRecordRequest},
        transmission::{
            codec::{TransmissionRequest, TransmissionResponse},
            PendingRequests,
        },
    },
    config::NetworkConfig,
    error::P2PError,
    event::ComposedEvent,
    runtime::handle_event::EventHandler,
    Behaviour, Command, Event, NotReadyMessage,
};
use libp2p::{
    core::transport::ListenerId,
    kad::{
        kbucket, record::Key, BootstrapOk, KademliaEvent, PutRecordError, QueryId, QueryResult,
        Quorum, Record,
    },
    request_response::{RequestResponseEvent, RequestResponseMessage},
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, Swarm,
};
use tokio::sync::{
    mpsc, oneshot,
    oneshot::{Receiver, Sender},
};
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

pub struct Runtime {
    pub(crate) config: NetworkConfig,
    pub(crate) peer_set: HashSet<PeerId>,
    pub(crate) swarm: Swarm<Behaviour>,
    pub(crate) command_receiver: mpsc::Receiver<Command>,
    pub(crate) event_sender: mpsc::Sender<Event>,
    pub(crate) local_peer_id: PeerId,
    pub(crate) listening_on: Multiaddr,
    #[allow(unused)]
    pub(crate) addresses: Multiaddr,
    pub(crate) bootstrapped: bool,
    pub(crate) is_boot_node: bool,

    pub(crate) pending_requests: PendingRequests,

    /// Contains peer ids of dialled node
    pub peers: HashSet<PeerId>,

    /// Holds the pending dials and their sender
    pub pending_dial: PendingDials,

    /// Contains current listenerId of the swarm
    pub active_listeners: HashSet<ListenerId>,

    /// Pending DHT queries
    pub pending_record_requests: HashMap<QueryId, PendingRecordRequest>,

    /// Shutdown signal receiver from the client
    pub(crate) shutdown: mpsc::Receiver<oneshot::Sender<()>>,
}

mod handle_command;
mod handle_event;

impl Runtime {
    fn start_listening(&mut self, peer_addr: Multiaddr) -> Result<(), P2PError> {
        self.swarm
            .listen_on(peer_addr)
            .map(|_| ())
            .map_err(Into::into)
    }

    pub async fn bootstrap(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        if self.bootstrapped {
            return Err(Box::new(P2PError::BootstrapError(
                "Network already boostrapped or in bootstrap",
            )));
        }

        self.bootstrapped = true;

        let addr = self.listening_on.clone();
        if let Err(error) = self.swarm.listen_on(addr) {
            error!(
                "Couldn't start listening on {} because of {error:?}",
                self.listening_on
            );

            return Err(Box::new(error));
        }

        debug!("Starting a boot node ? {:?}", self.is_boot_node);
        if !self.is_boot_node {
            // First we need to be known and known some peers before publishing our addresses to
            // the network.
            let mut publish_retry = self.config.publish_retry;

            // We were able to send the DHT query, starting the bootstrap
            // We may want to remove the bootstrap at some point
            if self.swarm.behaviour_mut().discovery.bootstrap().is_err() {
                return Err(Box::new(P2PError::BootstrapError(
                    "Unable to start kademlia bootstrap",
                )));
            }

            // The AddrAnnoucer is responsible of publishing those addresses
            let mut addr_query_id: Option<QueryId> = None;

            while let Some(event) = self.swarm.next().await {
                match event {
                    SwarmEvent::Behaviour(ComposedEvent::PeerInfo(event)) => {
                        info!("Received peer_info, {event:?}");
                        // Validate peer_info here
                        self.handle(event).await;
                        if self.peer_set.len() > self.config.minimum_cluster_size {
                            let key = Key::new(&self.local_peer_id.to_string());
                            addr_query_id = if let Ok(query_id_record) =
                                self.swarm.behaviour_mut().discovery.put_record(
                                    Record::new(key, self.addresses.to_vec()),
                                    Quorum::Majority,
                                ) {
                                Some(query_id_record)
                            } else {
                                return Err(Box::new(P2PError::BootstrapError(
                                    "Unable to send the addr Record to DHT",
                                )));
                            };
                        }
                    }
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::OutboundQueryProgressed {
                            id,
                            result:
                                QueryResult::PutRecord(Err(PutRecordError::QuorumFailed {
                                    key,
                                    success,
                                    quorum,
                                })),
                            stats,
                            ..
                        },
                    )) if Some(id) == addr_query_id && publish_retry == 0 => {
                        debug!("QuorumFailure on DHT addr publication: key: {key:?}, success: {success:?}, quorum: {quorum:?}, stats: {stats:?}");
                        return Err(Box::new(P2PError::BootstrapError(
                            "Unable to send the addr Record to DHT",
                        )));
                    }

                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::OutboundQueryProgressed {
                            id,
                            result: QueryResult::PutRecord(Err(PutRecordError::QuorumFailed { .. })),
                            ..
                        },
                    )) if Some(id) == addr_query_id && publish_retry > 0 => {
                        publish_retry -= 1;
                        warn!("Failed to PutRecord in DHT, retry again, attempt number {publish_retry}");
                        let key = Key::new(&self.local_peer_id.to_string());
                        if let Ok(query_id_record) = self
                            .swarm
                            .behaviour_mut()
                            .discovery
                            .put_record(Record::new(key, self.addresses.to_vec()), Quorum::Majority)
                        {
                            addr_query_id = Some(query_id_record);
                        } else {
                            return Err(Box::new(P2PError::BootstrapError(
                                "Unable to send the addr Record to DHT",
                            )));
                        }
                    }
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::OutboundQueryProgressed {
                            id,
                            result: QueryResult::PutRecord(Ok(_)),
                            ..
                        },
                    )) if Some(id) == addr_query_id => {
                        warn!(
                            "Bootstrap finished and MultiAddr published on DHT for {}",
                            self.local_peer_id
                        );

                        break;
                    }
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::OutboundQueryProgressed {
                            result: QueryResult::Bootstrap(Ok(BootstrapOk { .. })),
                            ..
                        },
                    )) => {}

                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::OutboundQueryProgressed { .. },
                    )) => {}

                    // Handle protocol queries
                    SwarmEvent::Behaviour(ComposedEvent::Transmission(
                        RequestResponseEvent::Message {
                            peer,
                            message:
                                RequestResponseMessage::Request {
                                    request_id,
                                    request,
                                    channel,
                                },
                        },
                    )) => {
                        info!("Received a protocol message from {peer}: id: {request_id}, {request:?}");
                        if self
                            .swarm
                            .behaviour_mut()
                            .transmission
                            .send_response(
                                channel,
                                TransmissionResponse(
                                    bincode::serialize(&NotReadyMessage {}).unwrap(),
                                ),
                            )
                            .is_err()
                        {
                            error!("Unable to send NotReadyMessage as response to {request_id}");
                        }
                    }

                    SwarmEvent::ConnectionEstablished { .. } => {}

                    SwarmEvent::Dialing(_) => {}
                    SwarmEvent::IncomingConnection { .. } => {}

                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::InboundRequest { .. },
                    )) => {}
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::RoutingUpdated { .. },
                    )) => {}
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::RoutablePeer { .. },
                    )) => {}
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::UnroutablePeer { .. },
                    )) => {}
                    event => warn!("Unhandle event during Bootstrap: {event:?}"),
                }
            }
        }

        warn!("Network bootstrap finished");

        Ok(self)
    }

    /// Run p2p runtime
    pub async fn run(mut self) -> Result<(), ()> {
        let shutdowned: Option<oneshot::Sender<()>> = loop {
            tokio::select! {
                Some(event) = self.swarm.next() => self.handle(event).await,
                Some(command) = self.command_receiver.recv() => self.handle_command(command).await,
                shutdown = self.shutdown.recv() => {
                    break shutdown;
                }
            }
        };

        if let Some(sender) = shutdowned {
            info!("Shutting down p2p runtime...");
            _ = sender.send(());
        }

        Ok(())
    }
}
