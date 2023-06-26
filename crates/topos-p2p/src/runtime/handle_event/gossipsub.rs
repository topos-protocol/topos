use libp2p::gossipsub::{Event as GossipsubEvent, Message};
use topos_metrics::{
    MESSAGE_RECEIVED_ON_ECHO, MESSAGE_RECEIVED_ON_GOSSIP, MESSAGE_RECEIVED_ON_READY,
};
use tracing::{error, info};

use crate::{Event, Runtime, TOPOS_ECHO, TOPOS_GOSSIP, TOPOS_READY};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<Box<GossipsubEvent>> for Runtime {
    async fn handle(&mut self, event: Box<GossipsubEvent>) {
        if let GossipsubEvent::Message {
            message:
                Message {
                    source: Some(source),
                    data,
                    topic,
                    ..
                },
            message_id,
            ..
        } = *event
        {
            info!(
                "Received message {:?} from {:?} on topic {:?}",
                message_id, source, topic
            );
            match topic.as_str() {
                TOPOS_GOSSIP => {
                    MESSAGE_RECEIVED_ON_GOSSIP.inc();
                }
                TOPOS_ECHO => {
                    MESSAGE_RECEIVED_ON_ECHO.inc();
                }
                TOPOS_READY => {
                    MESSAGE_RECEIVED_ON_READY.inc();
                }
                _ => {
                    error!("Received message on unknown topic {:?}", topic);
                }
            }

            if let Err(e) = self
                .event_sender
                .try_send(Event::Gossip { from: source, data })
            {
                tracing::error!("Failed to send gossip event to runtime: {:?}", e);
            }
        }
    }
}
