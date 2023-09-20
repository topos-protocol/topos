use tce_transport::{ProtocolEvents, TceCommands};
use tracing::{debug, error, info};

use crate::events::Events;
use crate::messages::NetworkMessage;
use crate::AppContext;

impl AppContext {
    pub async fn on_protocol_event(&mut self, evt: ProtocolEvents) {
        match evt {
            ProtocolEvents::StableSample => {
                info!("Stable Sample detected");
                self.api_client.set_active_sample(true).await;
                if self.events.send(Events::StableSample).await.is_err() {
                    error!("Unable to send StableSample event");
                }
            }

            ProtocolEvents::Broadcast { certificate_id } => {
                info!("Broadcasting certificate {}", certificate_id);
            }

            ProtocolEvents::Gossip { cert } => {
                let cert_id = cert.id;

                let data = NetworkMessage::from(TceCommands::OnGossip { cert });

                info!("Sending Gossip for certificate {}", cert_id);
                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_GOSSIP, data)
                    .await
                {
                    error!("Unable to send Gossip due to error: {e}");
                }
            }

            ProtocolEvents::Echo {
                certificate_id,
                signature,
                validator_id,
            } => {
                // Send echo message
                let data = NetworkMessage::from(TceCommands::OnEcho {
                    certificate_id,
                    signature,
                    validator_id,
                });

                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_ECHO, data)
                    .await
                {
                    error!("Unable to send Echo due to error: {e}");
                }
            }

            ProtocolEvents::Ready {
                certificate_id,
                signature,
                validator_id,
            } => {
                let data = NetworkMessage::from(TceCommands::OnReady {
                    certificate_id,
                    signature,
                    validator_id,
                });

                if let Err(e) = self
                    .network_client
                    .publish::<NetworkMessage>(topos_p2p::TOPOS_READY, data)
                    .await
                {
                    error!("Unable to send Ready due to error: {e}");
                }
            }

            evt => {
                debug!("Unhandled event: {:?}", evt);
            }
        }
    }
}
