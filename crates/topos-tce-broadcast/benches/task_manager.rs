use std::collections::HashSet;
use std::sync::Arc;
use tce_transport::ReliableBroadcastParams;
use tokio::sync::{broadcast, mpsc, oneshot};
use topos_tce_broadcast::double_echo::DoubleEcho;
use topos_tce_broadcast::sampler::SubscriptionsView;
use topos_tce_storage::authority::AuthorityStore;
use topos_test_sdk::certificates::create_certificate_chain;
use topos_test_sdk::constants::{SOURCE_SUBNET_ID_1, TARGET_SUBNET_ID_1};

const CHANNEL_SIZE: usize = 256_000;

struct TceParams {
    nb_peers: usize,
    broadcast_params: ReliableBroadcastParams,
}

pub async fn processing_double_echo(n: u64, authority_store: Arc<AuthorityStore>) {
    let (subscriptions_view_sender, subscriptions_view_receiver) = mpsc::channel(CHANNEL_SIZE);

    let (_cmd_sender, cmd_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (event_sender, _event_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (broadcast_sender, mut broadcast_receiver) = broadcast::channel(CHANNEL_SIZE);
    let (_double_echo_shutdown_sender, double_echo_shutdown_receiver) =
        mpsc::channel::<oneshot::Sender<()>>(1);
    let (task_manager_message_sender, task_manager_message_receiver) = mpsc::channel(CHANNEL_SIZE);

    let params = TceParams {
        nb_peers: 10,
        broadcast_params: ReliableBroadcastParams {
            echo_threshold: 8,
            ready_threshold: 5,
            delivery_threshold: 8,
        },
    };

    let mut double_echo = DoubleEcho::new(
        params.broadcast_params,
        task_manager_message_sender.clone(),
        cmd_receiver,
        event_sender,
        double_echo_shutdown_receiver,
        0,
        authority_store,
        broadcast_sender,
    );

    // List of peers
    let mut peers = HashSet::new();
    for i in 0..params.nb_peers {
        let peer = topos_p2p::utils::local_key_pair(Some(i as u8))
            .public()
            .to_peer_id();
        peers.insert(peer);
    }

    // Subscriptions
    double_echo.subscriptions.echo = peers.clone();
    double_echo.subscriptions.ready = peers.clone();
    double_echo.subscriptions.network_size = params.nb_peers;

    let msg = SubscriptionsView {
        echo: peers.clone(),
        ready: peers.clone(),
        network_size: params.nb_peers,
    };

    subscriptions_view_sender.send(msg).await.unwrap();

    double_echo.spawn_task_manager(subscriptions_view_receiver, task_manager_message_receiver);

    let certificates =
        create_certificate_chain(SOURCE_SUBNET_ID_1, &[TARGET_SUBNET_ID_1], n as usize);

    let double_echo_selected_echo = double_echo
        .subscriptions
        .echo
        .iter()
        .take(double_echo.params.echo_threshold)
        .cloned()
        .collect::<Vec<_>>();

    let double_echo_selected_ready = double_echo
        .subscriptions
        .ready
        .iter()
        .take(double_echo.params.delivery_threshold)
        .cloned()
        .collect::<Vec<_>>();

    for cert in &certificates {
        double_echo.broadcast(cert.certificate.clone(), true).await;
    }

    for cert in &certificates {
        for p in &double_echo_selected_echo {
            double_echo.handle_echo(*p, cert.certificate.id).await;
        }

        for p in &double_echo_selected_ready {
            double_echo.handle_ready(*p, cert.certificate.id).await;
        }
    }

    let mut count = 0;

    while let Ok(_event) = broadcast_receiver.recv().await {
        count += 1;

        if count == n {
            break;
        }
    }
}
