use std::collections::hash_map::Entry;

use crate::{
    behaviour::transmission::codec::{TransmissionRequest, TransmissionResponse},
    constant::SYNCHRONIZER_PROTOCOL,
    error::P2PError,
    Command, Runtime,
};
use libp2p::{
    gossipsub::IdentTopic,
    kad::{record::Key, Quorum},
    swarm::NetworkBehaviour,
    PeerId,
};
use topos_metrics::P2P_MESSAGE_SENT_ON_GOSSIPSUB_TOTAL;
use tracing::{debug, error, info, warn};
impl Runtime {
    pub(crate) async fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { peer_addr, sender } => {
                if sender.send(self.start_listening(peer_addr)).is_err() {
                    warn!("Unable to notify StartListening response: initiator is dropped");
                }
            }

            Command::ConnectedPeers { sender } => {
                if sender
                    .send(Ok(self
                        .swarm
                        .connected_peers()
                        .cloned()
                        .collect::<Vec<_>>()))
                    .is_err()
                {
                    warn!("Unable to notify ConnectedPeers response: initiator is dropped");
                }
            }

            Command::Dial {
                peer_id,
                peer_addr,
                sender,
            } if peer_id != *self.swarm.local_peer_id() => {
                info!("Sending an active Dial");
                // let handler = self.new_handler();
                match (self.peers.get(&peer_id), self.pending_dial.entry(peer_id)) {
                    (None, Entry::Vacant(entry)) => {
                        _ = self.swarm.dial(peer_addr);
                        entry.insert(sender);
                    }

                    _ => {
                        if sender.send(Err(P2PError::AlreadyDialed(peer_id))).is_err() {
                            warn!("Could not notify that {peer_id} was already dialed because initiator is dropped");
                        }
                    }
                }
            }

            Command::Dial { sender, .. } => {
                if sender.send(Err(P2PError::CantDialSelf)).is_err() {
                    warn!(
                        reason = %P2PError::CantDialSelf,
                        "Unable to notify Dial failure because initiator is dropped",
                    );
                }
            }

            Command::Disconnect { sender } if self.swarm.listeners().count() == 0 => {
                if sender.send(Err(P2PError::AlreadyDisconnected)).is_err() {
                    warn!(
                        reason = %P2PError::AlreadyDisconnected,
                        "Unable to notify Disconnection failure: initiator is dropped",
                    );
                }
            }

            Command::Disconnect { sender } => {
                // TODO: Listeners must be handled by topos behaviour not discovery
                let listeners = self.active_listeners.iter().cloned().collect::<Vec<_>>();

                listeners.iter().for_each(|listener| {
                    self.swarm.remove_listener(*listener);
                });

                let peers: Vec<PeerId> = self.swarm.connected_peers().cloned().collect();

                for peer_id in peers {
                    if self.swarm.disconnect_peer_id(peer_id).is_err() {
                        info!("Peer {peer_id} wasn't connected during Disconnection command");
                    }
                }

                if sender.send(Ok(())).is_err() {
                    warn!("Unable to notify Disconnection: initiator is dropped",);
                }
            }

            Command::Discover { to, sender } => {
                let behaviour = self.swarm.behaviour_mut();
                let addr = behaviour.discovery.get_addresses_of_peer(&to);

                info!("Checking if we know {to} -> KAD {:?}", addr);
                if addr.is_empty() {
                    info!("We don't know {to}, fetching its Multiaddr from DHT");
                    let query_id = behaviour
                        .discovery
                        .inner
                        .get_record(Key::new(&to.to_string()));

                    debug!("Created a get_record query {query_id:?} for discovering {to}");
                    if let Some(id) = self.pending_record_requests.insert(query_id, sender) {
                        warn!("Discover request {id:?} was overwritten by {query_id:?}");
                    }
                } else {
                    _ = sender.send(Ok(addr));
                }
            }

            Command::TransmissionReq {
                to,
                data,
                sender,
                protocol,
            } => {
                let request_id = match protocol {
                    SYNCHRONIZER_PROTOCOL => self
                        .swarm
                        .behaviour_mut()
                        .synchronizer
                        .send_request(&to, TransmissionRequest::Synchronizer(data)),
                    _ => self
                        .swarm
                        .behaviour_mut()
                        .transmission
                        .send_request(&to, TransmissionRequest::Transmission(data)),
                };

                info!("Created a transmission request {request_id:?} for {to}");

                if let Some(id) = self.pending_requests.insert(request_id, sender) {
                    warn!("Transmission request {id:?} was overwritten by {request_id:?}",);
                }
            }

            Command::TransmissionResponse {
                data,
                channel,
                protocol,
            } => {
                match protocol {
                    SYNCHRONIZER_PROTOCOL => {
                        _ = self
                            .swarm
                            .behaviour_mut()
                            .synchronizer
                            .send_response(channel, data.map(TransmissionResponse::Synchronizer))
                    }

                    _ => {
                        _ = self
                            .swarm
                            .behaviour_mut()
                            .transmission
                            .send_response(channel, data.map(TransmissionResponse::Transmission))
                    }
                };
            }

            Command::Gossip { topic, data } => {
                match self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                    Ok(message_id) => {
                        debug!("Published message to {topic}");
                        P2P_MESSAGE_SENT_ON_GOSSIPSUB_TOTAL.inc();
                    }
                    Err(err) => error!("Failed to publish message to {topic}: {err}"),
                }
            }
        }
    }
}
