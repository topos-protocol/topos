use std::collections::hash_map;

use tce_transport::TceCommands;
use tokio::spawn;
use topos_core::api::grpc::shared::v1::positions::SourceStreamPosition;
use topos_core::api::grpc::tce::v1::{
    CheckpointMapFieldEntry, CheckpointRequest, CheckpointResponse, FetchCertificatesRequest,
    FetchCertificatesResponse, ProofOfDelivery, SignedReady,
};
use topos_core::uci::CertificateId;
use topos_metrics::CERTIFICATE_DELIVERY_LATENCY;
use topos_p2p::constant::SYNCHRONIZER_PROTOCOL;
use topos_p2p::{Event as NetEvent, NetworkClient};
use topos_tce_broadcast::DoubleEchoCommand;
use topos_tce_storage::store::ReadStore;
use tracing::{error, info, trace};

use crate::messages::NetworkMessage;
use crate::AppContext;

impl AppContext {
    pub async fn on_net_event(&mut self, evt: NetEvent) {
        trace!(
            "on_net_event: peer: {} event {:?}",
            &self.network_client.local_peer_id,
            &evt
        );

        match evt {
            NetEvent::Gossip { from, data } => {
                let msg: NetworkMessage = data.into();

                if let NetworkMessage::Cmd(cmd) = msg {
                    match cmd {
                        TceCommands::OnGossip { cert } => {
                            let channel = self.tce_cli.get_double_echo_channel();
                            if let hash_map::Entry::Vacant(entry) =
                                self.delivery_latency.entry(cert.id)
                            {
                                entry.insert(CERTIFICATE_DELIVERY_LATENCY.start_timer());
                            }

                            spawn(async move {
                                info!("Send certificate to be broadcast");
                                if channel
                                    .send(DoubleEchoCommand::Broadcast {
                                        cert,
                                        need_gossip: false,
                                    })
                                    .await
                                    .is_err()
                                {
                                    error!(
                                        "Unable to send broadcast_new_certificate command, \
                                         Receiver was dropped"
                                    );
                                }
                            });
                        }

                        TceCommands::OnEcho { certificate_id } => {
                            let channel = self.tce_cli.get_double_echo_channel();
                            spawn(async move {
                                if let Err(e) = channel
                                    .send(DoubleEchoCommand::Echo {
                                        from_peer: from,
                                        certificate_id,
                                    })
                                    .await
                                {
                                    error!("Unable to send Echo, {:?}", e);
                                }
                            });
                        }
                        TceCommands::OnReady { certificate_id } => {
                            let channel = self.tce_cli.get_double_echo_channel();
                            spawn(async move {
                                if let Err(e) = channel
                                    .send(DoubleEchoCommand::Ready {
                                        from_peer: from,
                                        certificate_id,
                                    })
                                    .await
                                {
                                    error!("Unable to send Ready {:?}", e);
                                }
                            });
                        }
                        _ => {}
                    }
                }
            }
            NetEvent::SynchronizerRequest { data, channel, .. } => {
                let msg: Result<CheckpointRequest, _> = data.clone().try_into();
                if let Ok(msg) = msg {
                    let res: Result<Vec<_>, _> =
                        msg.checkpoint.into_iter().map(|v| v.try_into()).collect();

                    let res = match res {
                        Err(error) => {
                            error!("{}", error);
                            return;
                        }
                        Ok(value) => value,
                    };

                    let diff = if let Ok(diff) = self.validator_store.get_checkpoint_diff(res) {
                        diff.into_iter()
                            .map(|(key, value)| {
                                let v: Vec<_> = value
                                    .into_iter()
                                    .map(|v| ProofOfDelivery {
                                        delivery_position: Some(SourceStreamPosition {
                                            source_subnet_id: Some(
                                                v.delivery_position.subnet_id.into(),
                                            ),
                                            position: *v.delivery_position.position,
                                            certificate_id: Some(v.certificate_id.into()),
                                        }),
                                        readies: v
                                            .readies
                                            .into_iter()
                                            .map(|(ready, signature)| SignedReady {
                                                ready,
                                                signature,
                                            })
                                            .collect(),
                                        threshold: v.threshold,
                                    })
                                    .collect();
                                CheckpointMapFieldEntry {
                                    key: key.to_string(),
                                    value: v,
                                }
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };

                    let response = CheckpointResponse {
                        request_id: msg.request_id,
                        checkpoint_diff: diff,
                    };

                    let response: Vec<u8> = response.into();
                    _ = self
                        .network_client
                        .respond_to_request(Ok(response), channel, SYNCHRONIZER_PROTOCOL)
                        .await;
                } else {
                    let msg: Result<FetchCertificatesRequest, _> = data.try_into();
                    if let Ok(msg) = msg {
                        let certificate_ids: Vec<CertificateId> = msg
                            .certificates
                            .into_iter()
                            .map(|c| c.try_into().unwrap())
                            .collect();

                        if let Ok(certs) =
                            self.validator_store.get_certificates(&certificate_ids[..])
                        {
                            let certs: Vec<_> = certs
                                .into_iter()
                                .filter_map(|v| v.map(|c| c.certificate.try_into().unwrap()))
                                .collect();

                            let response = FetchCertificatesResponse {
                                request_id: msg.request_id,
                                certificates: certs,
                            };
                            _ = self
                                .network_client
                                .respond_to_request(Ok(response), channel, SYNCHRONIZER_PROTOCOL)
                                .await;
                        } else {
                            _ = self
                                .network_client
                                .respond_to_request::<FetchCertificatesResponse>(
                                    Err(()),
                                    channel,
                                    SYNCHRONIZER_PROTOCOL,
                                )
                                .await;
                        }
                    }
                }
            }

            NetEvent::TransmissionOnReq { .. } => {}
            _ => {}
        }
    }
}
