use std::collections::hash_map::Entry;

use crate::{
    error::{CommandExecutionError, P2PError},
    protocol_name, Command, Runtime,
};
use libp2p::{kad::record::Key, PeerId};
use rand::{seq::SliceRandom, thread_rng, Rng};
use topos_metrics::P2P_MESSAGE_SENT_ON_GOSSIPSUB_TOTAL;
use tracing::{debug, error, info, warn};

impl Runtime {
    pub(crate) async fn handle_command(&mut self, command: Command) {
        match command {
            Command::NewProxiedQuery {
                peer,
                id,
                response,
                protocol,
            } => {
                let connection = self
                    .swarm
                    .behaviour_mut()
                    .grpc
                    .open_outbound_connection(&peer, protocol_name!(protocol));

                _ = response.send(connection);
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
            Command::RandomKnownPeer { sender } => {
                if self.peer_set.is_empty() {
                    sender.send(Err(P2PError::CommandError(
                        CommandExecutionError::NoKnownPeer,
                    )));

                    return;
                }

                let selected_peer: usize = thread_rng().gen_range(0..(self.peer_set.len()));
                if sender
                    .send(
                        self.peer_set
                            .iter()
                            .nth(selected_peer)
                            .cloned()
                            .ok_or(P2PError::CommandError(CommandExecutionError::NoKnownPeer)),
                    )
                    .is_err()
                {
                    warn!("Unable to notify RandomKnownPeer response: initiator is dropped");
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

            Command::Gossip {
                topic,
                data: message,
            } => match self.swarm.behaviour_mut().gossipsub.publish(topic, message) {
                Ok(message_id) => {
                    debug!("Published message to {topic}");
                    P2P_MESSAGE_SENT_ON_GOSSIPSUB_TOTAL.inc();
                }
                Err(err) => error!("Failed to publish message to {topic}: {err}"),
            },
        }
    }
}
