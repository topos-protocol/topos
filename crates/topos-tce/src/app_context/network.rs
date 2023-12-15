use prost::Message;
use std::collections::hash_map;

use tokio::spawn;

use topos_metrics::CERTIFICATE_DELIVERY_LATENCY;
use topos_p2p::Event as NetEvent;
use topos_tce_broadcast::DoubleEchoCommand;
use tracing::{debug, error, info, trace};

use topos_core::api::grpc::tce::v1::{double_echo_request, DoubleEchoRequest, Echo, Gossip, Ready};
use topos_core::uci;

use crate::AppContext;

impl AppContext {
    pub async fn on_net_event(&mut self, evt: NetEvent) {
        trace!(
            "on_net_event: peer: {} event {:?}",
            &self.network_client.local_peer_id,
            &evt
        );

        let NetEvent::Gossip { data, from } = evt;
        if let Ok(DoubleEchoRequest {
            request: Some(double_echo_request),
        }) = DoubleEchoRequest::decode(&data[..])
        {
            match double_echo_request {
                double_echo_request::Request::Gossip(Gossip {
                    certificate: Some(certificate),
                }) => match uci::Certificate::try_from(certificate) {
                    Ok(cert) => {
                        let channel = self.tce_cli.get_double_echo_channel();
                        if let hash_map::Entry::Vacant(entry) = self.delivery_latency.entry(cert.id)
                        {
                            entry.insert(CERTIFICATE_DELIVERY_LATENCY.start_timer());
                        }
                        debug!(
                            "Received certificate {} from Gossip message from {}",
                            cert.id, from
                        );
                        spawn(async move {
                            info!("Send certificate {} to be broadcast", cert.id);
                            if channel
                                .send(DoubleEchoCommand::Broadcast {
                                    cert,
                                    need_gossip: false,
                                })
                                .await
                                .is_err()
                            {
                                error!(
                                    "Unable to send broadcast_new_certificate command, Receiver \
                                     was dropped"
                                );
                            }
                        });
                    }
                    Err(e) => {
                        error!("Error converting received certificate {e}");
                    }
                },
                double_echo_request::Request::Echo(Echo {
                    certificate_id: Some(certificate_id),
                    signature: Some(signature),
                    validator_id: Some(validator_id),
                }) => {
                    let channel = self.tce_cli.get_double_echo_channel();
                    spawn(async move {
                        let certificate_id = certificate_id.clone().try_into().map_err(|e| {
                            error!(
                                "Invalid certificate id, could not process Echo message: {e}, \
                                 certificate_id: {certificate_id}"
                            );
                            e
                        });
                        let validator_id = validator_id.clone().try_into().map_err(|e| {
                            error!(
                                "Invalid validator id, could not process Echo message: {e}, \
                                 validator_id: {validator_id}"
                            );
                            e
                        });

                        if let (Ok(certificate_id), Ok(validator_id)) =
                            (certificate_id, validator_id)
                        {
                            debug!(
                                "Received Echo message, certificate_id: {certificate_id}, \
                                 validator_id: {validator_id} from {from}",
                                certificate_id = certificate_id,
                                validator_id = validator_id
                            );
                            if let Err(e) = channel
                                .send(DoubleEchoCommand::Echo {
                                    signature: signature.into(),
                                    certificate_id,
                                    validator_id,
                                })
                                .await
                            {
                                error!("Unable to pass received Echo message, {:?}", e);
                            }
                        } else {
                            error!("Unable to process Echo message due to invalid data");
                        }
                    });
                }
                double_echo_request::Request::Ready(Ready {
                    certificate_id: Some(certificate_id),
                    signature: Some(signature),
                    validator_id: Some(validator_id),
                }) => {
                    let channel = self.tce_cli.get_double_echo_channel();
                    spawn(async move {
                        let certificate_id = certificate_id.clone().try_into().map_err(|e| {
                            error!(
                                "Invalid certificate id, could not process Ready message: {e}, \
                                 certificate_id: {certificate_id}"
                            );
                            e
                        });
                        let validator_id = validator_id.clone().try_into().map_err(|e| {
                            error!(
                                "Invalid validator id, could not process Ready message: {e}, \
                                 validator_id: {validator_id}"
                            );
                            e
                        });
                        if let (Ok(certificate_id), Ok(validator_id)) =
                            (certificate_id, validator_id)
                        {
                            debug!(
                                "Received Ready message, certificate_id: {certificate_id}, \
                                 validator_id: {validator_id} from {from}",
                                certificate_id = certificate_id,
                                validator_id = validator_id
                            );
                            if let Err(e) = channel
                                .send(DoubleEchoCommand::Ready {
                                    signature: signature.into(),
                                    certificate_id,
                                    validator_id,
                                })
                                .await
                            {
                                error!("Unable to pass received Ready message, {:?}", e);
                            }
                        } else {
                            error!("Unable to process Ready message due to invalid data");
                        }
                    });
                }
                _ => {}
            }
        }
    }
}
