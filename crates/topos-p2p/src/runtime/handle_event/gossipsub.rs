use libp2p::gossipsub::{Event as GossipsubEvent, Message};
use topos_metrics::{
    MESSAGE_RECEIVED_ON_ECHO, MESSAGE_RECEIVED_ON_GOSSIP, MESSAGE_RECEIVED_ON_READY,
};
use tracing::{error, info};

use crate::{
    behaviour::gossip::Batch, event::GossipEvent, Event, Runtime, TOPOS_ECHO, TOPOS_GOSSIP,
    TOPOS_READY,
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
            info!("Received message from {:?} on topic {:?}", source, topic);
            match topic {
                TOPOS_GOSSIP => {
                    MESSAGE_RECEIVED_ON_GOSSIP.inc();

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
                    MESSAGE_RECEIVED_ON_ECHO.inc();

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
                    MESSAGE_RECEIVED_ON_READY.inc();

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
