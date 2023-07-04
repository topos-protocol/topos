use libp2p::gossipsub::{Event as GossipsubEvent, Message};
use topos_metrics::{
    P2P_EVENT_STREAM_CAPACITY_TOTAL, P2P_MESSAGE_RECEIVED_ON_ECHO_TOTAL,
    P2P_MESSAGE_RECEIVED_ON_GOSSIP_TOTAL, P2P_MESSAGE_RECEIVED_ON_READY_TOTAL,
};
use tracing::{error, info};

use crate::{
    behaviour::gossip::Batch, constant, event::GossipEvent, Event, Runtime, TOPOS_ECHO,
    TOPOS_GOSSIP, TOPOS_READY,
};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<GossipEvent> for Runtime {
    async fn handle(&mut self, event: GossipEvent) {
        if let GossipEvent {
            source: Some(source),
            message,
            topic,
        } = event
        {
            if self.event_sender.capacity() < *constant::CAPACITY_EVENT_STREAM_BUFFER {
                P2P_EVENT_STREAM_CAPACITY_TOTAL.inc();
            }

            info!("Received message from {:?} on topic {:?}", source, topic);
            match topic {
                TOPOS_GOSSIP => {
                    P2P_MESSAGE_RECEIVED_ON_GOSSIP_TOTAL.inc();

                    if let Err(e) = self
                        .event_sender
                        .send(Event::Gossip {
                            from: source,
                            data: message,
                        })
                        .await
                    {
                        tracing::error!("Failed to send gossip event to runtime: {:?}", e);
                    }
                }
                TOPOS_ECHO => {
                    P2P_MESSAGE_RECEIVED_ON_ECHO_TOTAL.inc();

                    let msg: Batch = bincode::deserialize(&message).unwrap();

                    for data in msg.data {
                        if let Err(e) = self
                            .event_sender
                            .send(Event::Gossip { from: source, data })
                            .await
                        {
                            tracing::error!("Failed to send gossip event to runtime: {:?}", e);
                        }
                    }
                }
                TOPOS_READY => {
                    P2P_MESSAGE_RECEIVED_ON_READY_TOTAL.inc();

                    let msg: Batch = bincode::deserialize(&message).unwrap();

                    for data in msg.data {
                        if let Err(e) = self
                            .event_sender
                            .send(Event::Gossip { from: source, data })
                            .await
                        {
                            tracing::error!("Failed to send gossip event to runtime: {:?}", e);
                        }
                    }
                }
                _ => {
                    error!("Received message on unknown topic {:?}", topic);
                }
            }
        }
    }
}
