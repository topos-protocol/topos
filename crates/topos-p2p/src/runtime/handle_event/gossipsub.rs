use libp2p::gossipsub::{Event as GossipsubEvent, Message};
use tracing::info;

use crate::{Event, Runtime};

use super::EventHandler;

#[async_trait::async_trait]
impl EventHandler<Box<GossipsubEvent>> for Runtime {
    async fn handle(&mut self, event: Box<GossipsubEvent>) {
        match *event {
            GossipsubEvent::Message {
                message:
                    Message {
                        source,
                        data,
                        topic,
                        ..
                    },
                message_id,
                ..
            } => {
                if let Some(source) = source {
                    info!(
                        "Received message {:?} from {:?} on topic {:?}",
                        message_id, source, topic
                    );
                    if let Err(e) = self
                        .event_sender
                        .try_send(Event::Gossip { from: source, data })
                    {
                        tracing::error!("Failed to send gossip event to runtime: {:?}", e);
                    }
                }
            }
            _ => {}
        }
    }
}
