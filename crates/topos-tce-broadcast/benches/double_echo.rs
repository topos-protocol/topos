use std::collections::HashSet;
use tce_transport::{ProtocolEvents, ReliableBroadcastParams};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, oneshot};
use topos_tce_broadcast::double_echo::DoubleEcho;
use topos_tce_broadcast::sampler::SubscriptionsView;
use topos_tce_broadcast::DoubleEchoCommand;
use topos_test_sdk::certificates::create_certificate_chain;
use topos_test_sdk::constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1};

const CHANNEL_SIZE: usize = 256_000;

#[derive(Clone)]
struct TceParams {
    nb_peers: usize,
    broadcast_params: ReliableBroadcastParams,
}

struct Context {
    event_receiver: Receiver<ProtocolEvents>,
}

pub async fn processing_double_echo(n: u64) {
    let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, event_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (_double_echo_shutdown_sender, double_echo_shutdown_receiver) =
        mpsc::channel::<oneshot::Sender<()>>(1);

    let params = TceParams {
        nb_peers: 10,
        broadcast_params: ReliableBroadcastParams {
            echo_threshold: 8,
            ready_threshold: 5,
            delivery_threshold: 8,
        },
    };

    let mut ctx = Context { event_receiver };

    let double_echo = DoubleEcho::new(
        params.clone().broadcast_params,
        cmd_receiver,
        event_sender,
        double_echo_shutdown_receiver,
    );

    // List of peers
    let mut peers = HashSet::new();
    for i in 0..params.nb_peers {
        let peer = topos_p2p::utils::local_key_pair(Some(i as u8))
            .public()
            .to_peer_id();
        peers.insert(peer);
    }

    let msg = SubscriptionsView {
        echo: peers.clone(),
        ready: peers.clone(),
        network_size: params.nb_peers,
    };

    tokio::spawn(double_echo.run(subscriptions_view_receiver));

    subscriptions_view_sender.send(msg.clone()).await.unwrap();

    let certificates =
        create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], n as usize);

    let double_echo_selected_echo = msg
        .echo
        .iter()
        .take(params.broadcast_params.echo_threshold)
        .cloned()
        .collect::<Vec<_>>();

    let double_echo_selected_ready = msg
        .ready
        .iter()
        .take(params.broadcast_params.delivery_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for cert in &certificates {
        let _ = cmd_sender
            .send(DoubleEchoCommand::Broadcast {
                cert: cert.clone(),
                need_gossip: true,
            })
            .await;
    }

    for cert in &certificates {
        for p in &double_echo_selected_echo {
            let _ = cmd_sender
                .send(DoubleEchoCommand::Echo {
                    from_peer: *p,
                    certificate_id: cert.id,
                })
                .await;
        }

        for p in &double_echo_selected_ready {
            let _ = cmd_sender
                .send(DoubleEchoCommand::Ready {
                    from_peer: *p,
                    certificate_id: cert.id,
                })
                .await;
        }
    }

    let mut count = 0;

    while let Some(event) = ctx.event_receiver.recv().await {
        if let ProtocolEvents::CertificateDelivered { .. } = event {
            count += 1;

            if count == n {
                break;
            }
        }
    }
}
