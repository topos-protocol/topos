use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env,
    task::Poll,
    time::Duration,
};

use libp2p::{
    gossipsub::{self, IdentTopic, Message, MessageAuthenticity, MessageId},
    identity::Keypair,
    swarm::{NetworkBehaviour, THandlerInEvent, ToSwarm},
};
use prost::Message as ProstMessage;
use topos_core::api::grpc::tce::v1::Batch;
use topos_metrics::{P2P_DUPLICATE_MESSAGE_ID_RECEIVED_TOTAL, P2P_GOSSIP_BATCH_SIZE};
use tracing::{debug, error};

use crate::{constants, event::ComposedEvent, TOPOS_ECHO, TOPOS_GOSSIP, TOPOS_READY};

const MAX_BATCH_SIZE: usize = 10;

pub struct Behaviour {
    batch_size: usize,
    gossipsub: gossipsub::Behaviour,
    pending: HashMap<&'static str, VecDeque<Vec<u8>>>,
    tick: tokio::time::Interval,
    cache: HashSet<MessageId>,
}

impl Behaviour {
    pub fn publish(
        &mut self,
        topic: &'static str,
        message: Vec<u8>,
    ) -> Result<usize, &'static str> {
        match topic {
            TOPOS_GOSSIP => {
                if let Ok(msg_id) = self.gossipsub.publish(IdentTopic::new(topic), message) {
                    debug!("Published on topos_gossip: {:?}", msg_id);
                }
            }
            TOPOS_ECHO | TOPOS_READY => self.pending.entry(topic).or_default().push_back(message),
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

    pub async fn new(peer_key: Keypair) -> Self {
        let batch_size = env::var("TOPOS_GOSSIP_BATCH_SIZE")
            .map(|v| v.parse::<usize>())
            .unwrap_or(Ok(MAX_BATCH_SIZE))
            .unwrap();
        let gossipsub = gossipsub::ConfigBuilder::default()
            .max_transmit_size(2 * 1024 * 1024)
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(|msg_id| {
                // Content based id
                let mut s = DefaultHasher::new();
                msg_id.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_be_bytes())
            })
            .build()
            .unwrap();

        let gossipsub = gossipsub::Behaviour::new_with_metrics(
            MessageAuthenticity::Signed(peer_key),
            gossipsub,
            constants::METRIC_REGISTRY
                .lock()
                .await
                .sub_registry_with_prefix("libp2p_gossipsub"),
            Default::default(),
        )
        .unwrap();

        Self {
            batch_size,
            gossipsub,
            pending: [
                (TOPOS_ECHO, VecDeque::new()),
                (TOPOS_READY, VecDeque::new()),
            ]
            .into_iter()
            .collect(),
            tick: tokio::time::interval(Duration::from_millis(
                env::var("TOPOS_GOSSIP_INTERVAL")
                    .map(|v| v.parse::<u64>())
                    .unwrap_or(Ok(100))
                    .unwrap(),
            )),
            cache: HashSet::new(),
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
        if self.tick.poll_tick(cx).is_ready() {
            // Publish batch
            for (topic, queue) in self.pending.iter_mut() {
                if !queue.is_empty() {
                    let num_of_message = queue.len().min(self.batch_size);
                    let batch = Batch {
                        messages: queue.drain(0..num_of_message).collect(),
                    };

                    debug!("Publishing {} {}", batch.messages.len(), topic);
                    let msg = batch.encode_to_vec();
                    P2P_GOSSIP_BATCH_SIZE.observe(batch.messages.len() as f64);
                    match self.gossipsub.publish(IdentTopic::new(*topic), msg) {
                        Ok(message_id) => debug!("Published {} {}", topic, message_id),
                        Err(error) => error!("Failed to publish {}: {}", topic, error),
                    }
                }
            }
        }

        let event = match self.gossipsub.poll(cx, params) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(ToSwarm::GenerateEvent(event)) => Some(event),
            Poll::Ready(ToSwarm::ListenOn { opts }) => {
                return Poll::Ready(ToSwarm::ListenOn { opts })
            }
            Poll::Ready(ToSwarm::RemoveListener { id }) => {
                return Poll::Ready(ToSwarm::RemoveListener { id })
            }
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

        if let Some(gossipsub::Event::Message { ref message_id, .. }) = event {
            if self.cache.contains(message_id) {
                P2P_DUPLICATE_MESSAGE_ID_RECEIVED_TOTAL.inc();
            }
        }

        if let Some(gossipsub::Event::Message {
            propagation_source,
            message_id,
            message:
                Message {
                    source,
                    data,
                    sequence_number,
                    topic,
                },
        }) = event
        {
            match topic.as_str() {
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
            }
        }

        Poll::Pending
    }
}
