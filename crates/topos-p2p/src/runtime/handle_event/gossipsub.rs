use topos_metrics::{
    P2P_EVENT_STREAM_CAPACITY_TOTAL, P2P_MESSAGE_DESERIALIZE_FAILURE_TOTAL,
    P2P_MESSAGE_RECEIVED_ON_ECHO_TOTAL, P2P_MESSAGE_RECEIVED_ON_GOSSIP_TOTAL,
    P2P_MESSAGE_RECEIVED_ON_READY_TOTAL,
};
use tracing::{debug, error};

use crate::{constants, event::GossipEvent, Event, Runtime, TOPOS_ECHO, TOPOS_GOSSIP, TOPOS_READY};
use prost::Message;
use topos_core::api::grpc::tce::v1::Batch;

use super::{EventHandler, EventResult};

#[async_trait::async_trait]
impl EventHandler<GossipEvent> for Runtime {
    async fn handle(&mut self, event: GossipEvent) -> EventResult {
        if let GossipEvent::Message {
            source: Some(source),
            message,
            topic,
            message_id,
        } = event
        {
            if self.event_sender.capacity() < *constants::CAPACITY_EVENT_STREAM_BUFFER {
                P2P_EVENT_STREAM_CAPACITY_TOTAL.inc();
            }

            debug!("Received message from {:?} on topic {:?}", source, topic);
            match topic {
                TOPOS_GOSSIP => {
                    P2P_MESSAGE_RECEIVED_ON_GOSSIP_TOTAL.inc();

                    if let Err(e) = self
                        .event_sender
                        .send(Event::Gossip {
                            from: source,
                            data: message,
                            message_id: message_id.clone(),
                        })
                        .await
                    {
                        error!("Failed to send gossip event to runtime: {:?}", e);
                    }
                }
                TOPOS_ECHO | TOPOS_READY => {
                    if topic == TOPOS_ECHO {
                        P2P_MESSAGE_RECEIVED_ON_ECHO_TOTAL.inc();
                    } else {
                        P2P_MESSAGE_RECEIVED_ON_READY_TOTAL.inc();
                    }
                    if let Ok(Batch { messages }) = Batch::decode(&message[..]) {
                        for message in messages {
                            if let Err(e) = self
                                .event_sender
                                .send(Event::Gossip {
                                    from: source,
                                    data: message,
                                    message_id: message_id.clone(),
                                })
                                .await
                            {
                                error!("Failed to send gossip {} event to runtime: {:?}", topic, e);
                            }
                        }
                    } else {
                        P2P_MESSAGE_DESERIALIZE_FAILURE_TOTAL
                            .with_label_values(&[topic])
                            .inc();
                    }
                }
                _ => {
                    error!("Received message on unknown topic {:?}", topic);
                }
            }
        }

        Ok(())
    }
}
