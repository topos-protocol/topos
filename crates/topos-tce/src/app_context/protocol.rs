use tce_transport::ProtocolEvents;
use topos_core::api::grpc::tce::v1::{
    double_echo_request, DoubleEchoRequest, EchoRequest, GossipRequest, ReadyRequest,
};
use tracing::{debug, error, info};

use crate::events::Events;
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

                let request = DoubleEchoRequest {
                    request: Some(double_echo_request::Request::Gossip(GossipRequest {
                        certificate: Some(cert.into()),
                    })),
                };

                info!("Sending Gossip for certificate {}", cert_id);
                if let Err(e) = self
                    .network_client
                    .publish(topos_p2p::TOPOS_GOSSIP, request)
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
                let request = DoubleEchoRequest {
                    request: Some(double_echo_request::Request::Echo(EchoRequest {
                        certificate: Some(certificate_id.into()),
                        signature: Some(signature.into()),
                        validator_id: Some(validator_id.into()),
                    })),
                };

                if let Err(e) = self
                    .network_client
                    .publish(topos_p2p::TOPOS_ECHO, request)
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
                let request = DoubleEchoRequest {
                    request: Some(double_echo_request::Request::Ready(ReadyRequest {
                        certificate: Some(certificate_id.into()),
                        signature: Some(signature.into()),
                        validator_id: Some(validator_id.into()),
                    })),
                };

                if let Err(e) = self
                    .network_client
                    .publish(topos_p2p::TOPOS_READY, request)
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
