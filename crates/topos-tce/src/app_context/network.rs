use tce_transport::TceCommands;
use tokio::spawn;
use topos_p2p::Event as NetEvent;
use topos_tce_broadcast::DoubleEchoCommand;
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

        if let NetEvent::Gossip { from, data } = evt {
            let msg: NetworkMessage = data.into();

            if let NetworkMessage::Cmd(cmd) = msg {
                match cmd {
                    TceCommands::OnGossip { cert } => {
                        let channel = self.tce_cli.get_double_echo_channel();

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
                                error!("Unable to send broadcast_new_certificate command, Receiver was dropped");
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
    }
}
