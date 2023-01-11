use libp2p::{
    kad::{record::Key, KademliaEvent, QueryResult, Quorum, Record},
    Multiaddr,
};
use tracing::{error, info, warn};

use crate::{Event, Runtime};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<KademliaEvent> for Runtime {
    async fn handle(&mut self, event: KademliaEvent) {
        match event {
            KademliaEvent::RoutingUpdated {
                peer,
                is_new_peer: true,
                ..
            } if self.bootstrapped => {
                if self.peers.insert(peer) {
                    let peers = self.peers.iter().cloned().collect();
                    _ = self
                        .event_sender
                        .try_send(Event::PeersChanged { new_peers: peers });
                }
            }
            KademliaEvent::UnroutablePeer { peer } => {
                if self.peers.remove(&peer) {
                    let peers = self.peers.iter().cloned().collect();
                    _ = self
                        .event_sender
                        .try_send(Event::PeersChanged { new_peers: peers });
                }
            }
            KademliaEvent::OutboundQueryCompleted {
                result: QueryResult::Bootstrap(res),
                ..
            } => match res {
                Ok(_) => {
                    info!("Bootstrapping finished");
                    let key = Key::new(&self.local_peer_id.to_string());
                    _ = self
                        .swarm
                        .behaviour_mut()
                        .discovery
                        .put_record(Record::new(key, self.addresses.to_vec()), Quorum::All);
                }
                Err(e) => error!("Error: bootstrap : {e:?}"),
            },
            KademliaEvent::OutboundQueryCompleted {
                result: QueryResult::PutRecord(Err(e)),
                ..
            } => {
                error!("{e:?}");
            }
            KademliaEvent::OutboundQueryCompleted {
                result: QueryResult::GetRecord(res),
                id,
                ..
            } => match res {
                Ok(result) => {
                    if let Some(sender) = self.pending_record_requests.remove(&id) {
                        if let Some(peer_record) = result.records.first() {
                            if let Ok(addr) = Multiaddr::try_from(peer_record.record.value.clone())
                            {
                                if let Some(peer_id) = peer_record.record.publisher {
                                    if !sender.is_closed() {
                                        self.swarm
                                            .behaviour_mut()
                                            .discovery
                                            .add_address(&peer_id, addr.clone());

                                        if sender.send(Ok(vec![addr.clone()])).is_err() {
                                            // TODO: Hash the QueryId
                                            warn!("Could not notify Record query ({id:?}) response because initiator is dropped");
                                        }
                                    }
                                    self.swarm
                                        .behaviour_mut()
                                        .transmission
                                        .add_address(&peer_id, addr);
                                }
                            }
                        }
                    }
                }

                Err(error) => error!("{error:?}"),
            },
            _ => {}
        }
    }
}
