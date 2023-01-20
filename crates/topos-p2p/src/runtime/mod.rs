use std::collections::{HashMap, HashSet};

use crate::{
    behaviour::{
        discovery::{PendingDials, PendingRecordRequest},
        transmission::PendingRequests,
    },
    error::P2PError,
    event::ComposedEvent,
    runtime::handle_event::EventHandler,
    Behaviour, Command, Event,
};
use libp2p::{
    core::transport::ListenerId,
    kad::{
        kbucket, record::Key, BootstrapOk, KademliaEvent, PutRecordError, QueryId, QueryResult,
        Quorum, Record,
    },
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, Swarm,
};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{debug, error, info, warn};

pub struct Runtime {
    pub(crate) swarm: Swarm<Behaviour>,
    pub(crate) command_receiver: mpsc::Receiver<Command>,
    pub(crate) event_sender: mpsc::Sender<Event>,
    pub(crate) local_peer_id: PeerId,
    pub(crate) listening_on: Multiaddr,
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

    pub async fn run(mut self) {
        let addr = self.listening_on.clone();
        if let Err(error) = self.swarm.listen_on(addr) {
            error!(
                "Couldn't start listening on {} because of {error:?}",
                self.listening_on
            );
            std::process::exit(1);
        }

        debug!("Starting a boot node ? {:?}", self.is_boot_node);
        if !self.is_boot_node {
            // let query_id = match self.swarm.behaviour_mut().discovery.bootstrap() {
            // let query_id = match self.swarm.behaviour_mut().discovery.() {
            //     Ok(query_id) => query_id,
            //     Err(e) => {
            //         error!("Couldn't initiate DHT bootstrapping because of {e:?}");
            //         std::process::exit(1);
            //     }
            // };

            let mut publish_retry = 3;

            let key = Key::new(&self.local_peer_id.to_string());

            let mut addr_query_id = if let Ok(query_id_record) = self
                .swarm
                .behaviour_mut()
                .discovery
                .put_record(Record::new(key, self.addresses.to_vec()), Quorum::All)
            {
                Some(query_id_record)
            } else {
                error!("Unable to send the addr Record to DHT");

                std::process::exit(1);
            };

            while let Some(event) = self.swarm.next().await {
                match event {
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::OutboundQueryCompleted {
                            id,
                            result: QueryResult::PutRecord(Err(PutRecordError::QuorumFailed { .. })),
                            ..
                        },
                    )) if Some(id) == addr_query_id && publish_retry > 0 => {
                        publish_retry = publish_retry - 1;
                        warn!("Failed to PutRecord in DHT, retry again, attempt number {publish_retry}");
                        let key = Key::new(&self.local_peer_id.to_string());
                        if let Ok(query_id_record) = self
                            .swarm
                            .behaviour_mut()
                            .discovery
                            .put_record(Record::new(key, self.addresses.to_vec()), Quorum::All)
                        {
                            addr_query_id = Some(query_id_record);
                        } else {
                            error!("Unable to send the addr Record to DHT");

                            std::process::exit(1);
                        }
                    }
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::OutboundQueryCompleted {
                            id,
                            result: QueryResult::PutRecord(Ok(_)),
                            ..
                        },
                    )) if Some(id) == addr_query_id => {
                        warn!(
                            "Bootstrap finished and MultiAddr published on DHT for {}",
                            self.local_peer_id
                        );
                        self.bootstrapped = true;

                        break;
                    }

                    // SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                    //     KademliaEvent::OutboundQueryCompleted {
                    //         result:
                    //             QueryResult::Bootstrap(Ok(BootstrapOk {
                    //                 peer,
                    //                 num_remaining,
                    //             })),
                    //         id,
                    //         ..
                    //     },
                    // )) if id == query_id
                    //     && addr_query_id.is_none()
                    //     && peer == self.local_peer_id =>
                    // {
                    //     info!("Bootstrapping finished query_id: {id:?}, peer: {peer}, num_remaining: {num_remaining}");
                    //     let key = Key::new(&self.local_peer_id.to_string());
                    //     if let Ok(query_id_record) = self
                    //         .swarm
                    //         .behaviour_mut()
                    //         .discovery
                    //         .put_record(Record::new(key, self.addresses.to_vec()), Quorum::All)
                    //     {
                    //         addr_query_id = Some(query_id_record);
                    //     } else {
                    //         error!("Unable to send the addr Record to DHT");
                    //
                    //         std::process::exit(1);
                    //     }
                    // }
                    SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                        KademliaEvent::OutboundQueryCompleted {
                            result:
                                QueryResult::Bootstrap(Ok(BootstrapOk {
                                    peer,
                                    num_remaining,
                                })),
                            id,
                            ..
                        },
                    )) => {
                        let addr = self.swarm.behaviour_mut().addresses_of_peer(&peer);
                        warn!("WEIRD BOOTSTRAP: query_id: {id:?}, peer: {peer}, num_remaining: {num_remaining:?}, addr: {addr:?}");
                        let kad = &self.swarm.behaviour_mut().discovery;
                    }
                    event => warn!("Unhandle event during Bootstrap: {event:?}"),
                }
            }
        } else {
            self.bootstrapped = true;
        }

        debug!("Sending Bootstrapped event to runtime");
        _ = self.event_sender.send(Event::Bootstrapped).await;

        loop {
            tokio::select! {
                Some(event) = self.swarm.next() => self.handle(event).await,
                command = self.command_receiver.recv() =>
                    match command {
                        Some(command) => self.handle_command(command).await,
                        None => return,
                    }
            }
        }
    }
}
