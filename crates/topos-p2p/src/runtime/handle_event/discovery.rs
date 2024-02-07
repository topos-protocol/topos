use libp2p::{
    kad::{Event, GetRecordOk, QueryResult},
    Multiaddr,
};
use tracing::{debug, error, warn};

use crate::{error::CommandExecutionError, Runtime};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<Box<Event>> for Runtime {
    async fn handle(&mut self, event: Box<Event>) {
        match *event {
            Event::InboundRequest { request } => {
                // warn!("InboundRequest {:?}", request);
            }

            Event::RoutingUpdated {
                peer, addresses, ..
            } => {
                debug!("DHT -> RoutingUpdated {:?} {:?}", peer, addresses);
            }

            Event::RoutablePeer { peer, address } => {
                debug!("DHT -> RoutablePeer {:?}, {:?}", peer, address);
            }

            Event::PendingRoutablePeer { peer, address } => {
                debug!("DHT -> PendingRoutablePeer {:?}, {:?}", peer, address);
            }

            Event::UnroutablePeer { peer } => {
                // Ignored
            }

            Event::OutboundQueryProgressed {
                result: QueryResult::Bootstrap(res),
                id,
                ..
            } => {
                debug!("BootstrapResult query: {id:?},  {res:?}");
            }

            Event::OutboundQueryProgressed {
                result: QueryResult::PutRecord(Err(e)),
                id,
                ..
            } => {
                error!("PutRecord Failed query_id: {id:?}, error: {e:?}");
            }

            Event::OutboundQueryProgressed {
                result: QueryResult::GetRecord(res),
                id,
                ..
            } => match res {
                Ok(GetRecordOk::FoundRecord(result)) => {
                    debug!("GetRecordOk query: {id:?}, {result:?}");
                    if let Some(sender) = self.pending_record_requests.remove(&id) {
                        if let Ok(addr) = Multiaddr::try_from(result.record.value.clone()) {
                            if let Some(peer_id) = result.record.publisher {
                                if !sender.is_closed() {
                                    debug!("Adding {peer_id:?} address {addr:?} to DHT");
                                    self.swarm
                                        .behaviour_mut()
                                        .discovery
                                        .inner
                                        .add_address(&peer_id, addr.clone());

                                    if sender.send(Ok(vec![addr.clone()])).is_err() {
                                        // TODO: Hash the QueryId
                                        warn!(
                                            "Could not notify Record query ({id:?}) response \
                                             because initiator is dropped"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(GetRecordOk::FinishedWithNoAdditionalRecord { cache_candidates }) => {}

                Err(error) => {
                    if let Some(sender) = self.pending_record_requests.remove(&id) {
                        if sender
                            .send(Err(CommandExecutionError::DHTGetRecordFailed))
                            .is_err()
                        {
                            // TODO: Hash the QueryId
                            warn!(
                                "Could not notify Record query ({id:?}) response because \
                                 initiator is dropped"
                            );
                        }
                    }
                    warn!("GetRecordError query_id: {id:?}, error: {error:?}");
                }
            },

            Event::OutboundQueryProgressed {
                id, result, stats, ..
            } => {}
        }
    }
}
