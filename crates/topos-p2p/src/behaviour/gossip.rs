use std::{collections::VecDeque, env, task::Poll, time::Duration};

use libp2p::{
    gossipsub::{self, IdentTopic, Message, MessageAuthenticity, Topic},
    identity::Keypair,
    multihash::IdentityHasher,
    swarm::{NetworkBehaviour, THandlerInEvent, ToSwarm},
};
use serde::{Deserialize, Serialize};

use crate::{event::ComposedEvent, TOPOS_ECHO, TOPOS_GOSSIP, TOPOS_READY};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Batch {
    pub(crate) data: Vec<Vec<u8>>,
}

pub struct Behaviour {
    batch_size: usize,
    gossipsub: gossipsub::Behaviour,
    echo_queue: VecDeque<Vec<u8>>,
    ready_queue: VecDeque<Vec<u8>>,
    tick: tokio::time::Interval,
}

impl Behaviour {
    pub fn publish(&mut self, topic: &'static str, data: Vec<u8>) -> Result<usize, &'static str> {
        match topic {
            TOPOS_GOSSIP => {
                _ = self.gossipsub.publish(IdentTopic::new(topic), data);
            }
            TOPOS_ECHO => self.echo_queue.push_back(data),
            TOPOS_READY => self.ready_queue.push_back(data),
            _ => return Err("Invalid topic"),
        }

        Ok(0)
    }

    pub fn subscribe(&mut self) -> Result<(), &'static str> {
        self.gossipsub
            .subscribe(&gossipsub::IdentTopic::new(TOPOS_GOSSIP))
            .unwrap();

        self.gossipsub
            .subscribe(&gossipsub::IdentTopic::new(TOPOS_ECHO))
            .unwrap();

        self.gossipsub
            .subscribe(&gossipsub::IdentTopic::new(TOPOS_READY))
            .unwrap();

        Ok(())
    }

    pub fn new(peer_key: Keypair) -> Self {
        let batch_size = env::var("TOPOS_GOSSIP_BATCH_SIZE")
            .map(|v| v.parse::<usize>())
            .unwrap_or(Ok(10))
            .unwrap();
        let gossipsub = gossipsub::ConfigBuilder::default()
            .max_transmit_size(2 * 1024 * 1024)
            .validation_mode(gossipsub::ValidationMode::Strict)
            .build()
            .unwrap();

        let gossipsub =
            gossipsub::Behaviour::new(MessageAuthenticity::Signed(peer_key.clone()), gossipsub)
                .unwrap();

        Self {
            batch_size,
            gossipsub,
            echo_queue: VecDeque::new(),
            ready_queue: VecDeque::new(),
            tick: tokio::time::interval(Duration::from_millis(
                env::var("TOPOS_GOSSIP_INTERVAL")
                    .map(|v| v.parse::<u64>())
                    .unwrap_or(Ok(100))
                    .unwrap(),
            )),
        }
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = <gossipsub::Behaviour as NetworkBehaviour>::ConnectionHandler;

    type ToSwarm = ComposedEvent;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        peer: libp2p::PeerId,
        local_addr: &libp2p::Multiaddr,
        remote_addr: &libp2p::Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.gossipsub.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: libp2p::swarm::ConnectionId,
        peer: libp2p::PeerId,
        addr: &libp2p::Multiaddr,
        role_override: libp2p::core::Endpoint,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.gossipsub.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
        )
    }

    fn on_swarm_event(&mut self, event: libp2p::swarm::FromSwarm<Self::ConnectionHandler>) {
        self.gossipsub.on_swarm_event(event)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: libp2p::PeerId,
        connection_id: libp2p::swarm::ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        self.gossipsub
            .on_connection_handler_event(peer_id, connection_id, event)
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
        params: &mut impl libp2p::swarm::PollParameters,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if let Poll::Ready(_) = self.tick.poll_tick(cx) {
            // Publish batch
            if !self.echo_queue.is_empty() {
                let mut echos = Batch { data: Vec::new() };
                for _ in 0..self.batch_size {
                    if let Some(data) = self.echo_queue.pop_front() {
                        echos.data.push(data);
                    } else {
                        break;
                    }
                }

                let msg = bincode::serialize::<Batch>(&echos).expect("msg ser");

                _ = self.gossipsub.publish(IdentTopic::new(TOPOS_ECHO), msg);
            }

            if !self.ready_queue.is_empty() {
                let mut readies = Batch { data: Vec::new() };
                for _ in 0..self.batch_size {
                    if let Some(data) = self.ready_queue.pop_front() {
                        readies.data.push(data);
                    } else {
                        break;
                    }
                }

                let msg = bincode::serialize::<Batch>(&readies).expect("msg ser");

                _ = self.gossipsub.publish(IdentTopic::new(TOPOS_READY), msg);
            }
        }

        let event = match self.gossipsub.poll(cx, params) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(ToSwarm::GenerateEvent(event)) => event,
            Poll::Ready(ToSwarm::Dial { opts }) => return Poll::Ready(ToSwarm::Dial { opts }),
            Poll::Ready(ToSwarm::NotifyHandler {
                peer_id,
                handler,
                event,
            }) => {
                return Poll::Ready(ToSwarm::NotifyHandler {
                    peer_id,
                    handler,
                    event,
                })
            }
            Poll::Ready(ToSwarm::CloseConnection {
                peer_id,
                connection,
            }) => {
                return Poll::Ready(ToSwarm::CloseConnection {
                    peer_id,
                    connection,
                })
            }
            Poll::Ready(ToSwarm::ExternalAddrExpired(addr)) => {
                return Poll::Ready(ToSwarm::ExternalAddrExpired(addr))
            }
            Poll::Ready(ToSwarm::ExternalAddrConfirmed(addr)) => {
                return Poll::Ready(ToSwarm::ExternalAddrConfirmed(addr))
            }
            Poll::Ready(ToSwarm::NewExternalAddrCandidate(addr)) => {
                return Poll::Ready(ToSwarm::NewExternalAddrCandidate(addr))
            }
        };

        let outcome = match event {
            gossipsub::Event::Message {
                propagation_source,
                message_id,
                message:
                    Message {
                        source,
                        data,
                        sequence_number,
                        topic,
                    },
            } => match topic.as_str() {
                TOPOS_GOSSIP => {
                    return Poll::Ready(ToSwarm::GenerateEvent(ComposedEvent::Gossipsub(
                        crate::event::GossipEvent {
                            topic: TOPOS_GOSSIP,
                            message: data,
                            source,
                        },
                    )))
                }
                TOPOS_ECHO => {
                    return Poll::Ready(ToSwarm::GenerateEvent(ComposedEvent::Gossipsub(
                        crate::event::GossipEvent {
                            topic: TOPOS_ECHO,
                            message: data,
                            source,
                        },
                    )))
                }
                TOPOS_READY => {
                    return Poll::Ready(ToSwarm::GenerateEvent(ComposedEvent::Gossipsub(
                        crate::event::GossipEvent {
                            topic: TOPOS_READY,
                            message: data,
                            source,
                        },
                    )))
                }
                _ => {}
            },
            _ => {}
        };

        Poll::Pending
    }
}
